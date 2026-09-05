use anyhow::{bail, Result};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

pub const MAX_EVENTS: usize = 100_000;
pub const VIEW_LIMIT: usize = 10_000;
pub const DEFAULT_NETWORKS: &str = "10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub id: String,
    pub sensor_id: String,
    pub timestamp: i64,
    pub src_ip: String,
    pub dst_ip: String,
    #[serde(default)]
    pub src_mac: Option<String>,
    #[serde(default)]
    pub dst_mac: Option<String>,
    #[serde(default)]
    pub src_port: Option<u16>,
    #[serde(default)]
    pub dst_port: Option<u16>,
    pub protocol: String,
    pub bytes: u64,
    #[serde(default = "one")]
    pub packets: u64,
}
fn one() -> u64 {
    1
}

impl Observation {
    pub fn validate(&self) -> Result<()> {
        identifier(&self.id)?;
        identifier(&self.sensor_id)?;
        self.src_ip.parse::<IpAddr>()?;
        self.dst_ip.parse::<IpAddr>()?;
        if !(0..=253402300799).contains(&self.timestamp)
            || self.bytes > 1_000_000_000_000
            || self.packets > 1_000_000_000
            || self.protocol.is_empty()
            || self.protocol.len() > 40
        {
            bail!("Invalid observation bounds");
        }
        for value in [&self.src_mac, &self.dst_mac].into_iter().flatten() {
            if value.len() != 17
                || !value
                    .split(':')
                    .all(|p| p.len() == 2 && u8::from_str_radix(p, 16).is_ok())
            {
                bail!("Invalid MAC address");
            }
        }
        Ok(())
    }
}

pub fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 160
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c))
    {
        bail!(
            "Identifiers must contain 1–160 letters, digits, dots, colons, underscores or hyphens"
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sensor {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub interface: String,
    pub internet_coverage: String,
    pub lan_coverage: String,
    pub notes: String,
    pub last_seen: Option<i64>,
    pub status: String,
    pub dropped_packets: Option<u64>,
}
impl Sensor {
    pub fn new(id: &str, kind: &str) -> Self {
        Self {
            id: id.into(),
            name: id.into(),
            kind: kind.into(),
            interface: String::new(),
            internet_coverage: "unverified".into(),
            lan_coverage: "unverified".into(),
            notes: "Coverage depends on placement. No whole-network coverage has been verified."
                .into(),
            last_seen: None,
            status: "idle".into(),
            dropped_packets: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDetails {
    pub observed_at: Option<i64>,
    pub source: Option<String>,
    pub hostname: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub operating_system: Option<String>,
    #[serde(default)]
    pub services: Vec<ObservedService>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedService {
    pub port: u16,
    pub transport: String,
    pub name: Option<String>,
    pub product: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    #[serde(default)]
    pub details: DeviceDetails,
    pub id: String,
    pub name: String,
    pub addresses: Vec<String>,
    pub mac: Option<String>,
    pub category: String,
    pub identification: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub upload: u64,
    pub download: u64,
    pub local_bytes: u64,
    pub connections: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub src: String,
    pub dst: String,
    pub src_device: Option<String>,
    pub dst_device: Option<String>,
    pub port: Option<u16>,
    pub protocol: String,
    pub direction: String,
    pub bytes: u64,
    pub packets: u64,
    pub first_seen: i64,
    pub last_seen: i64,
    pub sensor_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub id: String,
    pub device_id: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub evidence: Vec<String>,
    pub timestamp: i64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub timestamp: i64,
    pub upload: u64,
    pub download: u64,
    pub local_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    pub upload: u64,
    pub download: u64,
    pub local_bytes: u64,
    pub packets: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub mode: String,
    pub sensors: Vec<Sensor>,
    pub selected_sensor: Option<String>,
    pub devices: Vec<Device>,
    pub conversations: Vec<Conversation>,
    pub alerts: Vec<Alert>,
    pub timeline: Vec<Bucket>,
    pub totals: Totals,
    pub networks: Vec<String>,
    pub observation_count: usize,
    pub retained_count: usize,
    pub limited: bool,
    pub generated_at: i64,
}

pub fn networks(input: &str) -> Result<Vec<IpNet>> {
    let nets: Vec<IpNet> = input
        .split(',')
        .map(str::trim)
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if nets.is_empty() || nets.len() > 64 {
        bail!("Provide 1–64 network CIDRs");
    }
    Ok(nets)
}

pub fn local(ip: &str, nets: &[IpNet]) -> bool {
    ip.parse::<IpAddr>()
        .is_ok_and(|ip| nets.iter().any(|net| net.contains(&ip)))
}

pub fn unicast(ip: &str) -> bool {
    match ip.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => !ip.is_multicast() && !ip.is_broadcast() && !ip.is_unspecified(),
        Ok(IpAddr::V6(ip)) => !ip.is_multicast() && !ip.is_unspecified(),
        _ => false,
    }
}

pub fn device_id(sensor: &str, ip: &str, mac: &Option<String>) -> String {
    // A sensor is an observation domain. Never merge identities across unrelated networks.
    let identity = mac
        .as_ref()
        .filter(|m| m != &&"00:00:00:00:00:00".to_string())
        .map(|m| m.to_lowercase())
        .unwrap_or_else(|| ip.into());
    format!("{sensor}:{identity}:{ip}")
}
