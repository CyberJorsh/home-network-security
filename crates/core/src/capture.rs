use crate::*;
use anyhow::{bail, Context, Result};
use std::{
    io::{BufRead, BufReader, Read},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub fn capture(
    store: &mut Store,
    sensor_id: &str,
    interface: &str,
    seconds: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize),
) -> Result<usize> {
    if !(1..=86400).contains(&seconds) {
        bail!("Capture duration must be 1–86400 seconds");
    }
    if interface.is_empty() || interface.len() > 256 || interface.starts_with('-') {
        bail!("Invalid interface");
    }
    let mut sensor = store
        .sensors()?
        .into_iter()
        .find(|s| s.id == sensor_id)
        .context("Unknown sensor")?;
    sensor.interface = interface.into();
    sensor.kind = "tshark".into();
    sensor.status = "collecting".into();
    sensor.dropped_packets = None;
    let mut child = tshark_command()
        .args(["-i", interface, "-a", &format!("duration:{seconds}")])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Install TShark and configure capture permissions first")?;
    if let Err(error) = store.set_sensor(&sensor) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let mut stderr = child.stderr.take().context("Missing diagnostics")?;
    let diagnostics = std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(n) = stderr.read(&mut chunk) {
            if n == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..n]);
            if output.len() > 65536 {
                output.drain(..output.len() - 65536);
            }
        }
        String::from_utf8_lossy(&output).into_owned()
    });
    let stdout = child.stdout.take().context("Missing capture stream")?;
    let (tx, rx) = mpsc::sync_channel(2048);
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let capture_id = uuid::Uuid::new_v4().to_string();
    let deadline = Instant::now() + Duration::from_secs(seconds + 10);
    let mut batch = Vec::new();
    let mut count = 0;
    let mut failure = None;
    let mut last_flush = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            break;
        }
        if Instant::now() > deadline {
            failure = Some("Capture timed out".to_string());
            break;
        }
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(line)) => match parse_tshark(&line, sensor_id, &capture_id) {
                Ok(Some(o)) => batch.push(o),
                Ok(None) => {}
                Err(e) => {
                    failure = Some(e.to_string());
                    break;
                }
            },
            Ok(Err(e)) => {
                failure = Some(e.to_string());
                break;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !batch.is_empty()
            && (batch.len() >= 256 || last_flush.elapsed() >= Duration::from_millis(250))
        {
            match store.ingest(&batch) {
                Ok(n) => {
                    count += n;
                    progress(count);
                }
                Err(e) => {
                    failure = Some(e.to_string());
                    break;
                }
            }
            batch.clear();
            last_flush = Instant::now();
        }
    }
    if failure.is_some() {
        let _ = child.kill();
    }
    drop(rx);
    let _ = reader.join();
    let status = child.wait()?;
    let diagnostic = diagnostics.join().unwrap_or_default();
    let stopped = cancel.load(Ordering::Relaxed);
    if !batch.is_empty() && failure.is_none() {
        count += store.ingest(&batch)?;
    }
    sensor.status = if (status.success() || stopped) && failure.is_none() {
        "stopped"
    } else {
        "error"
    }
    .into();
    sensor.last_seen = store
        .sensors()?
        .into_iter()
        .find(|s| s.id == sensor_id)
        .and_then(|s| s.last_seen);
    sensor.dropped_packets = if status.success() && failure.is_none() && !stopped {
        reported_drops(&diagnostic)
    } else {
        None
    };
    store.set_sensor(&sensor)?;
    if let Some(error) = failure {
        bail!("{error}");
    }
    if !status.success() && !stopped {
        bail!(
            "TShark capture failed. Check capture permissions: {}",
            diagnostic.chars().take(1800).collect::<String>()
        );
    }
    progress(count);
    Ok(count)
}

// Use only the capture engine's explicit final counter; absent/interrupted reports stay unknown.
pub fn reported_drops(diagnostic: &str) -> Option<u64> {
    diagnostic
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            if let (Some(count), Some(packet), Some("dropped")) =
                (words.next(), words.next(), words.next())
            {
                if packet == "packet" || packet == "packets" {
                    return count.parse::<u64>().ok();
                }
            }
            let rest = line.split_once("Packets received/dropped on interface ")?.1;
            let counts = rest.rsplit_once(": ")?.1.split_whitespace().next()?;
            let (_, dropped) = counts.split_once('/')?;
            dropped.parse::<u64>().ok()
        })
        .try_fold(None, |total: Option<u64>, n| {
            Some(Some(total.unwrap_or(0).checked_add(n)?))
        })
        .flatten()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drop_counts_require_explicit_reports() {
        assert_eq!(
            reported_drops(
                "Packets received/dropped on interface 'fixture0': 100/3 (pcap:3/dumpcap:0)"
            ),
            Some(3)
        );
        assert_eq!(
            reported_drops("Packets received/dropped on interface 'fixture0': 100/0 (100.0%)"),
            Some(0)
        );
        assert_eq!(reported_drops("3 packets dropped from fixture0"), Some(3));
        assert_eq!(reported_drops("1 packet dropped"), Some(1));
        assert_eq!(reported_drops("100 packets captured"), None);
        assert_eq!(reported_drops("Capture stopped"), None);
    }
}
