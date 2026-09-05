use crate::*;
use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    time::Duration,
};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        // Create private storage before SQLite creates its WAL and shared-memory files.
        let mut options = std::fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            options.mode(0o600);
            let file = options.open(path)?;
            file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        }
        #[cfg(not(unix))]
        let _ = options.open(path)?;
        Self::init(Connection::open(path)?)
    }
    pub fn memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }
    fn init(conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode=WAL;
          CREATE TABLE IF NOT EXISTS observations(id TEXT NOT NULL, sensor TEXT NOT NULL, ts INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(sensor,id));
          CREATE INDEX IF NOT EXISTS observations_time ON observations(sensor,ts);
          CREATE INDEX IF NOT EXISTS observations_retention ON observations(ts);
          CREATE TABLE IF NOT EXISTS sensors(id TEXT PRIMARY KEY, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS names(id TEXT PRIMARY KEY, name TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS discovery(sensor TEXT NOT NULL, ip TEXT NOT NULL, ts INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(sensor,ip));
          CREATE TABLE IF NOT EXISTS acknowledged(id TEXT PRIMARY KEY);
          CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        Ok(Self { conn })
    }
    pub fn set_networks(&self, value: &str) -> Result<()> {
        networks(value)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO settings VALUES ('networks',?1)",
            [value],
        )?;
        Ok(())
    }
    pub fn set_sensor(&self, sensor: &Sensor) -> Result<()> {
        identifier(&sensor.id)?;
        if sensor.name.len() > 128 || sensor.notes.len() > 2048 {
            bail!("Sensor metadata too long");
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO sensors VALUES (?1,?2)",
            params![sensor.id, serde_json::to_string(sensor)?],
        )?;
        Ok(())
    }
    pub fn sensors(&self) -> Result<Vec<Sensor>> {
        let mut stmt = self.conn.prepare("SELECT body FROM sensors ORDER BY id")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }
    pub fn ingest(&mut self, observations: &[Observation]) -> Result<usize> {
        if observations.len() > MAX_EVENTS {
            bail!("Import exceeds 100,000 observations; split it into batches");
        }
        let known: HashSet<String> = self.sensors()?.into_iter().map(|s| s.id).collect();
        for o in observations {
            o.validate()?;
            if !known.contains(&o.sensor_id) {
                bail!("Unknown sensor: {}", o.sensor_id);
            }
        }
        let tx = self.conn.transaction()?;
        let mut added = 0;
        {
            let mut stmt = tx.prepare("INSERT OR IGNORE INTO observations VALUES (?1,?2,?3,?4)")?;
            for o in observations {
                added += stmt.execute(params![
                    o.id,
                    o.sensor_id,
                    o.timestamp,
                    serde_json::to_string(o)?
                ])?;
            }
        }
        tx.execute("DELETE FROM observations WHERE rowid IN (SELECT rowid FROM observations ORDER BY ts DESC, rowid DESC LIMIT -1 OFFSET ?1)",[MAX_EVENTS as i64])?;
        tx.commit()?;
        for mut sensor in self.sensors()? {
            if let Some(last) = observations
                .iter()
                .filter(|o| o.sensor_id == sensor.id)
                .map(|o| o.timestamp)
                .max()
            {
                sensor.last_seen = Some(sensor.last_seen.unwrap_or(0).max(last));
                self.set_sensor(&sensor)?;
            }
        }
        Ok(added)
    }
    pub fn import_json(&mut self, input: &str, sensor: &str) -> Result<usize> {
        if input.len() > 32 * 1024 * 1024 {
            bail!("Import exceeds 32 MiB");
        }
        let observations = input
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|line| {
                let mut o: Observation = serde_json::from_str(line)?;
                o.sensor_id = sensor.into();
                Ok(o)
            })
            .collect::<Result<Vec<_>>>()?;
        self.ingest(&observations)
    }
    pub fn rename(&self, id: &str, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() || name.len() > 100 || name.chars().any(char::is_control) {
            bail!("Use a name of 1–100 characters without control characters");
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO names VALUES (?1,?2)",
            params![id, name],
        )?;
        Ok(())
    }
    pub fn acknowledge(&self, id: &str) -> Result<()> {
        if id.len() > 256 {
            bail!("Invalid alert ID");
        }
        self.conn
            .execute("INSERT OR IGNORE INTO acknowledged VALUES (?1)", [id])?;
        Ok(())
    }
    pub fn save_discovery(&self, sensor: &str, devices: &[DiscoveredDevice]) -> Result<()> {
        identifier(sensor)?;
        if !self.sensors()?.iter().any(|s| s.id == sensor) {
            bail!("Unknown sensor");
        }
        for device in devices {
            self.conn.execute(
                "INSERT OR REPLACE INTO discovery VALUES (?1,?2,?3,?4)",
                params![
                    sensor,
                    device.ip,
                    chrono::Utc::now().timestamp(),
                    serde_json::to_string(device)?
                ],
            )?;
        }
        Ok(())
    }
    pub fn snapshot(&self, selected: Option<&str>, mode: &str) -> Result<Snapshot> {
        let sensors = self.sensors()?;
        let selected = selected
            .map(String::from)
            .or_else(|| sensors.first().map(|s| s.id.clone()));
        if let Some(ref id) = selected {
            if !sensors.iter().any(|s| s.id == *id) {
                bail!("Unknown sensor");
            }
        }
        let mut stmt = self.conn.prepare(
            "SELECT body FROM observations WHERE sensor=?1 ORDER BY ts DESC,id DESC LIMIT ?2",
        )?;
        let observations: Vec<Observation> = stmt
            .query_map(params![selected, VIEW_LIMIT as i64], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str(&row?)?))
            .collect::<Result<_>>()?;
        let retained = self.conn.query_row(
            "SELECT COUNT(*) FROM observations WHERE sensor=?1",
            [&selected],
            |r| r.get::<_, i64>(0),
        )? as usize;
        let mut stmt = self.conn.prepare("SELECT id,name FROM names")?;
        let names: HashMap<String, String> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut stmt = self.conn.prepare("SELECT id FROM acknowledged")?;
        let acknowledged: HashSet<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        let configured = self
            .conn
            .query_row("SELECT value FROM settings WHERE key='networks'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap_or(DEFAULT_NETWORKS.into());
        let nets = networks(&configured)?;
        let mut devices: BTreeMap<String, Device> = BTreeMap::new();
        let mut discovered_stmt = self
            .conn
            .prepare("SELECT ts,body FROM discovery WHERE sensor=?1 ORDER BY ip LIMIT 4096")?;
        for row in discovered_stmt.query_map([&selected], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (ts, body) = row?;
            let found: DiscoveredDevice = serde_json::from_str(&body)?;
            let id = device_id(selected.as_deref().unwrap_or(""), &found.ip, &found.mac);
            let name = names
                .get(&id)
                .cloned()
                .or_else(|| found.hostname.clone())
                .unwrap_or(found.ip.clone());
            devices.insert(id.clone(),Device {id,name,addresses:vec![found.ip],mac:found.mac,category:"Unknown".into(),identification:format!("Nmap discovery; reported vendor: {}. Hostnames and vendors are hints, not verified identity.",found.vendor.unwrap_or("unknown".into())),first_seen:ts,last_seen:ts,upload:0,download:0,local_bytes:0,connections:0});
        }
        let mut conversations: BTreeMap<String, Conversation> = BTreeMap::new();
        let mut timeline: BTreeMap<i64, Bucket> = BTreeMap::new();
        let mut totals = Totals::default();
        for o in &observations {
            let src_local = local(&o.src_ip, &nets) && unicast(&o.src_ip);
            let dst_local = local(&o.dst_ip, &nets) && unicast(&o.dst_ip);
            let direction = if !unicast(&o.dst_ip) {
                "multicast"
            } else {
                match (src_local, dst_local) {
                    (true, true) => "local",
                    (true, false) => "upload",
                    (false, true) => "download",
                    _ => "transit",
                }
            };
            let src_id = src_local.then(|| device_id(&o.sensor_id, &o.src_ip, &o.src_mac));
            let dst_id = dst_local.then(|| device_id(&o.sensor_id, &o.dst_ip, &o.dst_mac));
            for (id, ip, mac, source) in [
                (&src_id, &o.src_ip, &o.src_mac, true),
                (&dst_id, &o.dst_ip, &o.dst_mac, false),
            ] {
                if let Some(id) = id {
                    let d = devices.entry(id.clone()).or_insert_with(|| Device {
                        id: id.clone(),
                        name: names.get(id).cloned().unwrap_or_else(|| ip.clone()),
                        addresses: vec![],
                        mac: mac.clone(),
                        category: "Unknown".into(),
                        identification: if names.contains_key(id) {
                            "Named by you".into()
                        } else if mac.is_some() {
                            "Observed address; type unknown".into()
                        } else {
                            "IP only; identity may change".into()
                        },
                        first_seen: o.timestamp,
                        last_seen: o.timestamp,
                        upload: 0,
                        download: 0,
                        local_bytes: 0,
                        connections: 0,
                    });
                    if !d.addresses.contains(ip) {
                        d.addresses.push(ip.clone());
                    }
                    d.first_seen = d.first_seen.min(o.timestamp);
                    d.last_seen = d.last_seen.max(o.timestamp);
                    match direction {
                        "upload" if source => d.upload += o.bytes,
                        "download" if !source => d.download += o.bytes,
                        "local" => d.local_bytes += o.bytes,
                        _ => {}
                    }
                }
            }
            let key = format!(
                "{}|{}|{:?}|{}|{:?}|{}",
                o.sensor_id, o.src_ip, o.src_port, o.dst_ip, o.dst_port, o.protocol
            );
            let id = format!("flow-{:x}", Sha256::digest(key.as_bytes()));
            let c = conversations.entry(key).or_insert_with(|| Conversation {
                id,
                src: o.src_ip.clone(),
                dst: o.dst_ip.clone(),
                src_device: src_id,
                dst_device: dst_id,
                port: o.dst_port,
                protocol: o.protocol.clone(),
                direction: direction.into(),
                bytes: 0,
                packets: 0,
                first_seen: o.timestamp,
                last_seen: o.timestamp,
                sensor_id: o.sensor_id.clone(),
            });
            c.bytes += o.bytes;
            c.packets += o.packets;
            c.first_seen = c.first_seen.min(o.timestamp);
            c.last_seen = c.last_seen.max(o.timestamp);
            let bucket_ts = o.timestamp / 300 * 300;
            let b = timeline.entry(bucket_ts).or_insert(Bucket {
                timestamp: bucket_ts,
                upload: 0,
                download: 0,
                local_bytes: 0,
            });
            match direction {
                "upload" => {
                    totals.upload += o.bytes;
                    b.upload += o.bytes;
                }
                "download" => {
                    totals.download += o.bytes;
                    b.download += o.bytes;
                }
                "local" => {
                    totals.local_bytes += o.bytes;
                    b.local_bytes += o.bytes;
                }
                _ => {}
            }
            totals.packets += o.packets;
        }
        for c in conversations.values() {
            for id in [&c.src_device, &c.dst_device].into_iter().flatten() {
                if let Some(d) = devices.get_mut(id) {
                    d.connections += 1;
                }
            }
        }
        let mut alerts = Vec::new();
        for d in devices.values() {
            let id = format!("device:{}", d.id);
            alerts.push(Alert { acknowledged:acknowledged.contains(&id), id,device_id:d.id.clone(),severity:"info".into(),title:"Device observed".into(),
                detail:"This device appears in retained observations. This does not establish when it joined your network or whether it is trusted.".into(),
                evidence:conversations.values().filter(|c|c.src_device.as_ref()==Some(&d.id)||c.dst_device.as_ref()==Some(&d.id)).take(3).map(|c|c.id.clone()).collect(),timestamp:d.first_seen });
            if d.upload >= 50 * 1024 * 1024 {
                let id = format!("upload:{}", d.id);
                alerts.push(Alert { acknowledged:acknowledged.contains(&id),id,device_id:d.id.clone(),severity:"notice".into(),title:"Large observed upload".into(),
                    detail:"At least 50 MiB sent to addresses outside your configured local networks in this view. Backups and video calls can explain this; it is not a malware finding.".into(),
                    evidence:conversations.values().filter(|c|c.src_device.as_ref()==Some(&d.id)&&c.direction=="upload").take(5).map(|c|c.id.clone()).collect(),timestamp:d.last_seen });
            }
        }
        alerts.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
        let mut conversations: Vec<_> = conversations.into_values().collect();
        conversations.sort_by_key(|c| std::cmp::Reverse(c.bytes));
        Ok(Snapshot {
            mode: mode.into(),
            sensors,
            selected_sensor: selected,
            devices: devices.into_values().collect(),
            conversations,
            alerts,
            timeline: timeline.into_values().collect(),
            totals,
            networks: nets.iter().map(ToString::to_string).collect(),
            observation_count: observations.len(),
            retained_count: retained,
            limited: retained > VIEW_LIMIT,
            generated_at: chrono::Utc::now().timestamp(),
        })
    }
}

pub fn demo_store() -> Result<Store> {
    let mut store = Store::memory()?;
    let mut sensor = Sensor::new("sample", "demo");
    sensor.name = "Sample home".into();
    sensor.status = "sample".into();
    sensor.notes = "Entirely synthetic fixture. It does not describe your network.".into();
    store.set_sensor(&sensor)?;
    store.import_json(include_str!("../../../fixtures/sample.ndjson"), "sample")?;
    for (last, name) in [
        ("11", "Living room TV"),
        ("12", "Work laptop"),
        ("13", "Kitchen speaker"),
        ("14", "Home server"),
        ("15", "Front door camera"),
        ("16", "Personal phone"),
    ] {
        store.rename(
            &format!("sample:02:00:00:00:00:{last}:10.42.0.{last}"),
            name,
        )?;
    }
    Ok(store)
}
