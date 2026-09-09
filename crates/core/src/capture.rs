use crate::*;
use anyhow::{bail, Context, Result};
use std::{
    io::{self, BufRead, BufReader, Read},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

const MAX_CAPTURE_LINE_BYTES: usize = 16 * 1024;

fn read_capture_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_CAPTURE_LINE_BYTES as u64 + 1)
        .read_until(b'\n', &mut bytes)?;
    if bytes.is_empty() {
        return Ok(None);
    }
    if bytes.len() > MAX_CAPTURE_LINE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Capture output line exceeds 16 KiB",
        ));
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

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
    let mut command = tshark_command();
    command.args(["-i", interface, "-a", &format!("duration:{seconds}")]);
    run_capture(store, sensor, seconds, cancel, &mut progress, &mut command)
}

fn run_capture(
    store: &mut Store,
    mut sensor: Sensor,
    seconds: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(usize),
    command: &mut Command,
) -> Result<usize> {
    if cancel.load(Ordering::Relaxed) {
        bail!("Collection cancelled");
    }
    let sensor_id = sensor.id.clone();
    sensor.status = "collecting".into();
    sensor.dropped_packets = None;
    let mut child = command
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
        let mut stream = BufReader::new(stdout);
        loop {
            match read_capture_line(&mut stream) {
                Ok(Some(line)) => {
                    if tx.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = tx.send(Err(error));
                    break;
                }
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
            Ok(Ok(line)) => match parse_tshark(&line, &sensor_id, &capture_id) {
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
                    // Do not retry a failed batch during cleanup.
                    batch.clear();
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
    if reader.join().is_err() {
        failure.get_or_insert_with(|| "Capture output reader failed".into());
    }
    let success = match child.wait() {
        Ok(status) => status.success(),
        Err(error) => {
            failure.get_or_insert_with(|| format!("Could not finish capture: {error}"));
            false
        }
    };
    let diagnostic = diagnostics.join().unwrap_or_default();
    let stopped = cancel.load(Ordering::Relaxed);
    // Parsed observations remain valid even when later output is malformed.
    if !batch.is_empty() {
        match store.ingest(&batch) {
            Ok(n) => count += n,
            Err(error) => {
                failure.get_or_insert_with(|| error.to_string());
            }
        }
    }
    progress(count);
    sensor = store
        .sensors()?
        .into_iter()
        .find(|s| s.id == sensor_id)
        .unwrap_or(sensor);
    sensor.status = if (success || stopped) && failure.is_none() {
        "stopped"
    } else {
        "error"
    }
    .into();
    sensor.dropped_packets = if success && failure.is_none() && !stopped {
        reported_drops(&diagnostic)
    } else {
        None
    };
    store.set_sensor(&sensor)?;
    if let Some(error) = failure {
        bail!("{error}");
    }
    if !success && !stopped {
        bail!(
            "TShark capture failed. Check capture permissions: {}",
            diagnostic.chars().take(1800).collect::<String>()
        );
    }
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
    fn capture_lines_have_a_memory_bound_and_keep_a_final_unterminated_line() {
        let mut input = io::Cursor::new(vec![b'x'; MAX_CAPTURE_LINE_BYTES * 4]);
        let error = read_capture_line(&mut input).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(input.position(), MAX_CAPTURE_LINE_BYTES as u64 + 1);
        let mut input = io::Cursor::new(b"first\nlast");
        assert_eq!(
            read_capture_line(&mut input).unwrap().as_deref(),
            Some("first\n")
        );
        assert_eq!(
            read_capture_line(&mut input).unwrap().as_deref(),
            Some("last")
        );
        assert_eq!(read_capture_line(&mut input).unwrap(), None);
    }

    #[test]
    #[cfg(unix)]
    fn keeps_valid_observations_before_a_malformed_capture_line() {
        let mut store = Store::memory().unwrap();
        let sensor = Sensor::new("fixture", "tshark");
        store.set_sensor(&sensor).unwrap();
        let mut progress = Vec::new();
        let result = run_capture(
            &mut store,
            sensor,
            1,
            &AtomicBool::new(false),
            |count| progress.push(count),
            Command::new("printf").arg("1\t1780000000.123\t10.42.0.2\t\t1.1.1.1\t\t\t\t1234\t\t443\t\tTLS\t128\nmalformed\n"),
        );
        assert!(result.unwrap_err().to_string().contains("14 TShark fields"));
        assert_eq!(progress.last(), Some(&1));
        assert_eq!(
            store.export_local().unwrap()["observations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(store.sensors().unwrap()[0].status, "error");
    }

    #[test]
    #[cfg(unix)]
    fn failed_final_ingest_marks_the_sensor_as_error() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("fixture.sqlite");
        let mut store = Store::open(&db).unwrap();
        let sensor = Sensor::new("fixture", "tshark");
        store.set_sensor(&sensor).unwrap();
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TRIGGER fixture_failure BEFORE INSERT ON observations BEGIN SELECT RAISE(ABORT, 'fixture storage failure'); END;").unwrap();
        let result = run_capture(
            &mut store,
            sensor,
            1,
            &AtomicBool::new(false),
            |_| {},
            Command::new("printf")
                .arg("1\t1780000000.123\t10.42.0.2\t\t1.1.1.1\t\t\t\t1234\t\t443\t\tTLS\t128\n"),
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("fixture storage failure"));
        assert_eq!(store.sensors().unwrap()[0].status, "error");
    }

    #[test]
    fn an_already_cancelled_capture_is_not_started() {
        let mut store = Store::memory().unwrap();
        let sensor = Sensor::new("fixture", "tshark");
        store.set_sensor(&sensor).unwrap();
        let result = run_capture(
            &mut store,
            sensor,
            1,
            &AtomicBool::new(true),
            |_| {},
            &mut Command::new("nonexistent-hns-fixture-command"),
        );
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert_ne!(store.sensors().unwrap()[0].status, "collecting");
    }

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
