use crate::*;
use anyhow::{bail, Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub const TSHARK_FIELDS: [&str; 14] = [
    "frame.number",
    "frame.time_epoch",
    "ip.src",
    "ipv6.src",
    "ip.dst",
    "ipv6.dst",
    "eth.src",
    "eth.dst",
    "tcp.srcport",
    "udp.srcport",
    "tcp.dstport",
    "udp.dstport",
    "_ws.col.Protocol",
    "frame.len",
];

pub fn tshark_command() -> Command {
    let mut command = tool_command("tshark");
    command.args(["-n", "-l", "-T", "fields", "-E", "occurrence=f"]);
    for field in TSHARK_FIELDS {
        command.args(["-e", field]);
    }
    command
}

pub fn parse_tshark(line: &str, sensor: &str, capture_id: &str) -> Result<Option<Observation>> {
    let f: Vec<_> = line.trim_end_matches(['\r', '\n']).split('\t').collect();
    if f.len() != 14 {
        bail!("Expected 14 TShark fields, got {}", f.len());
    }
    let src = if f[2].is_empty() { f[3] } else { f[2] };
    let dst = if f[4].is_empty() { f[5] } else { f[4] };
    // Non-IP frames do not become invented IP conversations.
    if src.is_empty() || dst.is_empty() {
        return Ok(None);
    }
    let seconds = f[1]
        .split('.')
        .next()
        .context("Missing timestamp")?
        .parse()?;
    let port = |a: &str, b: &str| -> Result<Option<u16>> {
        let value = if a.is_empty() { b } else { a };
        if value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(value.parse()?))
        }
    };
    let o = Observation {
        id: format!("{capture_id}-{}", f[0]),
        sensor_id: sensor.into(),
        timestamp: seconds,
        src_ip: src.into(),
        dst_ip: dst.into(),
        src_mac: (!f[6].is_empty()).then(|| f[6].into()),
        dst_mac: (!f[7].is_empty()).then(|| f[7].into()),
        src_port: port(f[8], f[9])?,
        dst_port: port(f[10], f[11])?,
        protocol: f[12].into(),
        bytes: f[13].parse()?,
        packets: 1,
    };
    o.validate()?;
    Ok(Some(o))
}

pub fn import_pcap(store: &mut Store, path: &Path, sensor: &str) -> Result<usize> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > 256 * 1024 * 1024 {
        bail!("PCAP import is limited to 256 MiB; split larger captures first");
    }
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let capture_id = format!("{:x}", hasher.finalize());
    let mut command = tshark_command();
    command.arg("-r").arg(path);
    let output = bounded_output(&mut command, 64 * 1024 * 1024, Duration::from_secs(120))
        .context("PCAP import requires separately installed TShark")?;
    let mut observations = Vec::new();
    for line in BufReader::new(output.as_slice()).lines() {
        if let Some(o) = parse_tshark(&line?, sensor, &capture_id)? {
            observations.push(o);
        }
        if observations.len() > MAX_EVENTS {
            bail!("Capture exceeds 100,000 IP packets; split it before import");
        }
    }
    store.ingest(&observations)
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn bounded_output(command: &mut Command, limit: usize, timeout: Duration) -> Result<Vec<u8>> {
    bounded_output_cancellable(
        command,
        limit,
        timeout,
        &std::sync::atomic::AtomicBool::new(false),
    )
}
pub fn bounded_output_cancellable(
    command: &mut Command,
    limit: usize,
    timeout: Duration,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<u8>> {
    let mut child = ChildGuard(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child.0.stdout.take().context("Missing tool output")?;
    let stderr = child.0.stderr.take().context("Missing tool diagnostics")?;
    let (tx, rx) = mpsc::channel();
    for (is_error, stream, cap) in [
        (false, Box::new(stdout) as Box<dyn Read + Send>, limit),
        (true, Box::new(stderr) as Box<dyn Read + Send>, 64 * 1024),
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stream
                .take(cap as u64 + 1)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send((is_error, cap, result));
        });
    }
    drop(tx);
    let deadline = Instant::now() + timeout;
    let mut output = Vec::new();
    let mut diagnostics = Vec::new();
    for _ in 0..2 {
        let (is_error, cap, result) = loop {
            if cancel.load(std::sync::atomic::Ordering::Relaxed) {
                bail!("Collection cancelled");
            }
            if Instant::now() >= deadline {
                bail!("Tool timed out while reading output");
            }
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(value) => break value,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("Tool output stream closed"),
            }
        };
        let bytes = result?;
        if bytes.len() > cap {
            bail!("Tool output exceeds import limit");
        }
        if is_error {
            diagnostics = bytes;
        } else {
            output = bytes;
        }
    }
    loop {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Collection cancelled");
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                bail!(
                    "Tool failed: {}",
                    String::from_utf8_lossy(&diagnostics)
                        .chars()
                        .take(1000)
                        .collect::<String>()
                );
            }
            return Ok(output);
        }
        if Instant::now() >= deadline {
            bail!("Tool timed out before exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveredDevice {
    pub ip: String,
    pub mac: Option<String>,
    pub hostname: Option<String>,
    pub vendor: Option<String>,
}

pub fn parse_nmap(xml: &str) -> Result<Vec<DiscoveredDevice>> {
    if xml.len() > 16 * 1024 * 1024 {
        bail!("Nmap XML exceeds 16 MiB");
    }
    let mut reader = Reader::from_str(xml);
    let mut devices = Vec::new();
    let mut ips = Vec::new();
    let mut mac = None;
    let mut hostname = None;
    let mut vendor = None;
    let mut up = false;
    let mut host = false;
    let mut depth: usize = 0;
    let mut root_seen = false;
    loop {
        let event = reader.read_event()?;
        match &event {
            Event::Start(e) | Event::Empty(e) => {
                if depth == 0 {
                    if root_seen || e.name().as_ref() != b"nmaprun" {
                        bail!("Expected one nmaprun root");
                    }
                    root_seen = true;
                }
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).context("Unexpected XML close tag")?;
            }
            Event::Eof if !root_seen || depth != 0 => bail!("Incomplete Nmap XML"),
            _ => {}
        }
        match event {
            Event::Start(e) if e.name().as_ref() == b"host" => {
                host = true;
                ips.clear();
                mac = None;
                hostname = None;
                vendor = None;
                up = false;
            }
            Event::Empty(e) if host => {
                let attrs = e
                    .attributes()
                    .map(|a| {
                        let a = a?;
                        Ok((
                            String::from_utf8_lossy(a.key.as_ref()).to_string(),
                            a.unescape_value()?.into_owned(),
                        ))
                    })
                    .collect::<Result<std::collections::HashMap<_, _>>>()?;
                let get = |key: &str| attrs.get(key).map(String::as_str).unwrap_or("");
                match e.name().as_ref() {
                    b"status" => up = get("state") == "up",
                    b"address" => match get("addrtype") {
                        "ipv4" | "ipv6" => {
                            let ip = get("addr");
                            ip.parse::<std::net::IpAddr>()?;
                            ips.push(ip.to_string());
                        }
                        "mac" => {
                            mac = Some(get("addr").to_lowercase());
                            vendor = attrs.get("vendor").cloned();
                        }
                        _ => {}
                    },
                    b"hostname" if hostname.is_none() => hostname = attrs.get("name").cloned(),
                    _ => {}
                }
            }
            Event::End(e) if e.name().as_ref() == b"host" => {
                if up {
                    for ip in &ips {
                        devices.push(DiscoveredDevice {
                            ip: ip.clone(),
                            mac: mac.clone(),
                            hostname: hostname.clone(),
                            vendor: vendor.clone(),
                        });
                    }
                }
                host = false;
            }
            Event::DocType(e) if e.as_ref() == b"nmaprun" => {}
            Event::DocType(_) => bail!("External or internal DTD declarations are not accepted"),
            Event::Eof => break,
            _ => {}
        }
    }
    if devices.len() > 4096 {
        bail!("Discovery exceeds 4,096 addresses; narrow the input");
    }
    Ok(devices)
}

pub fn discover(cidr: &str) -> Result<Vec<DiscoveredDevice>> {
    discover_cancellable(cidr, &std::sync::atomic::AtomicBool::new(false))
}
pub fn discover_cancellable(
    cidr: &str,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Vec<DiscoveredDevice>> {
    let nets = networks(cidr)?;
    if nets.len() != 1 {
        bail!("Discover one explicitly selected IPv4 network at a time");
    }
    match nets[0] {
        ipnet::IpNet::V4(net) if net.prefix_len() >= 24 && net.network().is_private() => {}
        _ => bail!("Discovery is limited to private IPv4 networks of /24 or smaller"),
    }
    let mut command = tool_command("nmap");
    command.args([
        "-sn",
        "-n",
        "--max-retries",
        "1",
        "--host-timeout",
        "10s",
        "-oX",
        "-",
        cidr,
    ]);
    let output = bounded_output_cancellable(
        &mut command,
        16 * 1024 * 1024,
        Duration::from_secs(300),
        cancel,
    )
    .context("Discovery requires separately installed Nmap")?;
    parse_nmap(&String::from_utf8(output)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn cancelling_a_tool_stops_it_promptly() {
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let signal = cancel.clone();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            signal.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let start = Instant::now();
        let result = bounded_output_cancellable(
            Command::new("sleep").arg("30"),
            1024,
            Duration::from_secs(60),
            &cancel,
        );
        assert!(result.unwrap_err().to_string().contains("cancelled"));
        assert!(start.elapsed() < Duration::from_secs(2));
        worker.join().unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn external_tool_output_and_runtime_are_bounded() {
        let mut command = Command::new("printf");
        command.arg("12345");
        assert!(bounded_output(&mut command, 4, Duration::from_secs(1)).is_err());
        let mut command = Command::new("printf");
        command.arg("1234");
        assert_eq!(
            bounded_output(&mut command, 4, Duration::from_secs(1)).unwrap(),
            b"1234"
        );
        let mut command = Command::new("sleep");
        command.arg("2");
        let started = Instant::now();
        assert!(bounded_output(&mut command, 4, Duration::from_millis(50)).is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
    }
    #[test]
    fn parses_ipv6_and_preserves_frame_bytes() {
        let o=parse_tshark("8\t1780000000.123\t\tfd00::1\t\t2606:4700::1111\t02:00:00:00:00:11\t02:00:00:00:00:01\t\t3000\t\t53\tDNS\t128","a","cap").unwrap().unwrap();
        assert_eq!(o.src_ip, "fd00::1");
        assert_eq!(o.dst_port, Some(53));
        assert_eq!(o.bytes, 128);
    }
    #[test]
    fn nmap_preserves_evidence_and_ignores_down_hosts() {
        let xml = r#"<?xml version="1.0"?><!DOCTYPE nmaprun><nmaprun><host><status state="up"/><address addr="10.1.1.2" addrtype="ipv4"/><address addr="02:00:00:00:00:11" addrtype="mac" vendor="Example"/><hostnames><hostname name="speaker.local"/></hostnames></host><host><status state="down"/><address addr="10.1.1.3" addrtype="ipv4"/></host></nmaprun>"#;
        let result = parse_nmap(xml).unwrap();
        assert!(parse_nmap("<nmaprun><host>").is_err());
        assert!(parse_nmap("<other/>").is_err());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hostname.as_deref(), Some("speaker.local"));
        assert!(
            parse_nmap("<!DOCTYPE x [<!ENTITY ext SYSTEM 'file:///etc/passwd'>]><nmaprun/>")
                .is_err()
        );
    }
}
