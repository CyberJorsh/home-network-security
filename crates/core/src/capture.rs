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
    let stderr = child.stderr.take().context("Missing diagnostics")?;
    let diagnostics = std::thread::spawn(move || {
        let mut output = Vec::new();
        let _ = stderr.take(65536).read_to_end(&mut output);
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
