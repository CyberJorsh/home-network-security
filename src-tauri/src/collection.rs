use hns_core::*;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub running: bool,
    pub kind: String,
    pub count: usize,
    pub sensor_id: Option<String>,
    pub error: Option<String>,
}
pub struct Collection {
    db: PathBuf,
    pub job: Arc<Mutex<Job>>,
    cancel: Arc<AtomicBool>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    id: String,
    label: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    interfaces: Vec<Interface>,
    addresses: Vec<String>,
    suggested_cidrs: Vec<String>,
    capture_error: Option<String>,
    discovery_available: bool,
    platform: String,
}
pub fn inspect() -> Result<Host, String> {
    let result = bounded_output(
        tool_command("tshark").arg("-D"),
        65536,
        Duration::from_secs(15),
    );
    let (interfaces, capture_error) = match result {
        Ok(bytes) => (
            String::from_utf8_lossy(&bytes)
                .lines()
                .filter_map(|line| {
                    let (_, label) = line.split_once(". ")?;
                    // Capture by the concrete interface name, not a reorderable numeric index.
                    let id = label.split(" (").next()?.to_string();
                    Some(Interface {
                        id,
                        label: label.into(),
                    })
                })
                .collect(),
            None,
        ),
        Err(e) => (Vec::new(), Some(format!("TShark unavailable: {e}"))),
    };
    let mut addresses = Vec::new();
    let mut suggested_cidrs = Vec::new();
    for interface in if_addrs::get_if_addrs().map_err(|e| e.to_string())? {
        if interface.is_loopback() {
            continue;
        }
        let name = &interface.name;
        addresses.push(format!("{name}: {}", interface.ip()));
        if let if_addrs::IfAddr::V4(addr) = interface.addr {
            if addr.ip.is_private() {
                let prefix = u32::from(addr.netmask).count_ones().max(24);
                let network = u32::from(addr.ip) & (u32::MAX << (32 - prefix));
                let cidr = format!("{}/{prefix}", std::net::Ipv4Addr::from(network));
                if !suggested_cidrs.contains(&cidr) {
                    suggested_cidrs.push(cidr);
                }
            }
        }
    }
    Ok(Host {
        interfaces,
        addresses,
        suggested_cidrs,
        capture_error,
        discovery_available: bounded_output(
            tool_command("nmap").arg("--version"),
            65536,
            Duration::from_secs(10),
        )
        .is_ok(),
        platform: std::env::consts::OS.into(),
    })
}
impl Collection {
    pub fn new(db: PathBuf) -> Self {
        Self {
            db,
            job: Arc::new(Mutex::new(Job::default())),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn shutdown(&self) {
        self.stop();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while self.job.lock().is_ok_and(|job| job.running) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn start(&self, kind: &str, target: String, seconds: u64) -> Result<String, String> {
        if kind != "discover" && kind != "capture" {
            return Err("Unknown collection operation".into());
        }
        if target.trim().is_empty() {
            return Err("Choose an interface or network first".into());
        }
        let mut job = self.job.lock().map_err(|e| e.to_string())?;
        if job.running {
            return Err("A collection job is already running".into());
        }
        let sensor_id = if kind == "discover" {
            "host-discovery".into()
        } else {
            use std::hash::{Hash, Hasher};
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            target.hash(&mut hash);
            format!("host-{:x}", hash.finish())
        };
        *job = Job {
            running: true,
            kind: kind.into(),
            sensor_id: Some(sensor_id.clone()),
            ..Job::default()
        };
        self.cancel.store(false, Ordering::Relaxed);
        let (db, status, cancel, kind, id) = (
            self.db.clone(),
            self.job.clone(),
            self.cancel.clone(),
            kind.to_string(),
            sensor_id.clone(),
        );
        std::thread::spawn(move || {
            let result = (|| -> anyhow::Result<usize> {
                if kind == "capture" {
                    let host = inspect().map_err(anyhow::Error::msg)?;
                    anyhow::ensure!(
                        host.interfaces.iter().any(|i| i.id == target),
                        "Choose a currently listed local capture interface"
                    );
                }
                let mut store = Store::open(&db)?;
                if !store.sensors()?.iter().any(|s| s.id == id) {
                    let mut sensor =
                        Sensor::new(&id, if kind == "discover" { "nmap" } else { "tshark" });
                    sensor.name = if kind == "discover" {
                        "This computer · discovery".into()
                    } else {
                        format!("This computer · {target}")
                    };
                    store.set_sensor(&sensor)?;
                }
                if kind == "discover" {
                    let found = discover_cancellable(&target, &cancel)?;
                    store.save_discovery(&id, &found)?;
                    Ok(found.len())
                } else {
                    capture(&mut store, &id, &target, seconds, &cancel, |count| {
                        if let Ok(mut job) = status.lock() {
                            job.count = count;
                        }
                    })
                }
            })();
            if let Ok(mut job) = status.lock() {
                job.running = false;
                match result {
                    Ok(count) => job.count = count,
                    Err(e) => job.error = Some(format!("{e:#}")),
                }
            }
        });
        Ok(sensor_id)
    }
}
impl Drop for Collection {
    fn drop(&mut self) {
        self.shutdown();
    }
}
