use crate::*;
use anyhow::{bail, Result};
use rusqlite::{params, Connection, OptionalExtension};
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
          CREATE TABLE IF NOT EXISTS endpoint_identity(sensor TEXT NOT NULL, mac TEXT NOT NULL, ip TEXT NOT NULL, first INTEGER NOT NULL, last INTEGER NOT NULL, PRIMARY KEY(sensor,mac,ip));
          CREATE TABLE IF NOT EXISTS discovery_first(sensor TEXT NOT NULL, ip TEXT NOT NULL, mac TEXT NOT NULL, ts INTEGER NOT NULL, PRIMARY KEY(sensor,ip,mac));
          CREATE TABLE IF NOT EXISTS acknowledged(id TEXT PRIMARY KEY);
          CREATE TABLE IF NOT EXISTS explanation_history(id TEXT PRIMARY KEY, saved INTEGER NOT NULL, body TEXT NOT NULL);
          CREATE TABLE IF NOT EXISTS settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        // Backfill existing installations without changing their observations or labels.
        if conn
            .query_row(
                "SELECT value FROM settings WHERE key='identity_schema'",
                [],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .is_none()
        {
            conn.execute_batch("BEGIN IMMEDIATE; INSERT OR IGNORE INTO endpoint_identity
          SELECT sensor,lower(json_extract(body,'$.srcMac')),json_extract(body,'$.srcIp'),min(ts),max(ts) FROM observations WHERE json_extract(body,'$.srcMac') IS NOT NULL GROUP BY sensor,lower(json_extract(body,'$.srcMac')),json_extract(body,'$.srcIp');
          INSERT INTO endpoint_identity SELECT sensor,lower(json_extract(body,'$.dstMac')),json_extract(body,'$.dstIp'),min(ts),max(ts) FROM observations WHERE json_extract(body,'$.dstMac') IS NOT NULL GROUP BY sensor,lower(json_extract(body,'$.dstMac')),json_extract(body,'$.dstIp')
          ON CONFLICT(sensor,mac,ip) DO UPDATE SET first=min(first,excluded.first),last=max(last,excluded.last);
          INSERT OR IGNORE INTO discovery_first SELECT sensor,ip,coalesce(lower(json_extract(body,'$.mac')),''),ts FROM discovery;
          INSERT INTO endpoint_identity SELECT sensor,lower(json_extract(body,'$.mac')),ip,ts,ts FROM discovery WHERE json_extract(body,'$.mac') IS NOT NULL ON CONFLICT(sensor,mac,ip) DO UPDATE SET first=min(first,excluded.first),last=max(last,excluded.last);
          INSERT OR REPLACE INTO settings VALUES ('identity_schema','1'); COMMIT;")?;
        }
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
        let limit = self.storage_limit()?;
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
                for (ip, mac) in [(&o.src_ip, &o.src_mac), (&o.dst_ip, &o.dst_mac)] {
                    if let Some(mac) = usable_mac(mac) {
                        tx.execute("INSERT INTO endpoint_identity VALUES (?1,?2,?3,?4,?4) ON CONFLICT(sensor,mac,ip) DO UPDATE SET first=min(first,excluded.first),last=max(last,excluded.last)", params![o.sensor_id,mac,ip,o.timestamp])?;
                    }
                }
            }
        }
        tx.execute("DELETE FROM observations WHERE rowid IN (SELECT rowid FROM observations ORDER BY ts DESC, rowid DESC LIMIT -1 OFFSET ?1)",[limit as i64])?;
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
            .take(MAX_EVENTS + 1)
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
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO names VALUES (?1,?2)",
            params![id, name],
        )?;
        // Keep address aliases for existing members in case later evidence reveals a shared MAC.
        for sensor in self.sensors()? {
            if let Some(mac) = id.strip_prefix(&format!("{}:mac:", sensor.id)) {
                let mut stmt =
                    tx.prepare("SELECT ip FROM endpoint_identity WHERE sensor=?1 AND mac=?2")?;
                let addresses = stmt
                    .query_map(params![sensor.id, mac], |r| r.get::<_, String>(0))?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for ip in addresses {
                    tx.execute(
                        "INSERT OR REPLACE INTO names VALUES (?1,?2)",
                        params![device_id(&sensor.id, &ip, &Some(mac.into())), name],
                    )?;
                }
            }
        }
        tx.commit()?;
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
        if devices.len() > 4096 {
            bail!("Discovery exceeds 4,096 addresses");
        }
        let tx = self.conn.unchecked_transaction()?;
        let now = chrono::Utc::now().timestamp();
        for device in devices {
            let mut device = device.clone();
            let previous: Option<String> = tx
                .query_row(
                    "SELECT body FROM discovery WHERE sensor=?1 AND ip=?2",
                    params![sensor, device.ip],
                    |r| r.get(0),
                )
                .optional()?;
            device.details.observed_at = Some(now);
            device.details.hostname = device.details.hostname.or(device.hostname.clone());
            device.details.vendor = device.details.vendor.or(device.vendor.clone());
            for (field, present) in [
                ("hostname", device.details.hostname.is_some()),
                ("vendor", device.details.vendor.is_some()),
                ("model", device.details.model.is_some()),
                ("operatingSystem", device.details.operating_system.is_some()),
            ] {
                if present {
                    device.details.field_observed_at.insert(field.into(), now);
                }
            }
            for service in &mut device.details.services {
                service.observed_at = Some(now);
            }
            if let Some(previous) = previous {
                let previous: DiscoveredDevice = serde_json::from_str(&previous)?;
                // An address reused by another MAC must not inherit the previous device's evidence.
                if usable_mac(&previous.mac).is_some()
                    && usable_mac(&previous.mac) == usable_mac(&device.mac)
                {
                    device.details = merge_details(previous.details, device.details);
                    device.hostname = device.hostname.or(previous.hostname);
                    device.vendor = device.vendor.or(previous.vendor);
                }
            }
            tx.execute(
                "INSERT OR IGNORE INTO discovery_first VALUES (?1,?2,?3,?4)",
                params![
                    sensor,
                    device.ip,
                    usable_mac(&device.mac).unwrap_or_default(),
                    now
                ],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO discovery VALUES (?1,?2,?3,?4)",
                params![sensor, device.ip, now, serde_json::to_string(&device)?],
            )?;
            if let Some(mac) = usable_mac(&device.mac) {
                tx.execute("INSERT INTO endpoint_identity VALUES (?1,?2,?3,?4,?4) ON CONFLICT(sensor,mac,ip) DO UPDATE SET first=min(first,excluded.first),last=max(last,excluded.last)",params![sensor,mac,device.ip,now])?;
            }
        }
        tx.commit()?;
        if let Some(mut current) = self.sensors()?.into_iter().find(|s| s.id == sensor) {
            current.last_seen = Some(chrono::Utc::now().timestamp());
            current.status = "discovery complete".into();
            self.set_sensor(&current)?;
        }
        Ok(())
    }
    pub fn snapshot(&self, selected: Option<&str>, mode: &str) -> Result<Snapshot> {
        self.snapshot_since(selected, mode, None)
    }
    pub fn snapshot_since(
        &self,
        selected: Option<&str>,
        mode: &str,
        since: Option<i64>,
    ) -> Result<Snapshot> {
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
            "SELECT body FROM observations WHERE sensor=?1 AND (?2 IS NULL OR ts>=?2) ORDER BY ts DESC,id DESC",
        )?;
        let observations: Vec<Observation> = stmt
            .query_map(params![selected, since], |row| row.get::<_, String>(0))?
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
        let identities = self.identities(selected.as_deref().unwrap_or(""), &nets, &names)?;
        let mut devices: BTreeMap<String, Device> = BTreeMap::new();
        let mut discovered_stmt = self
            .conn
            .prepare("SELECT ts,body FROM discovery WHERE sensor=?1 ORDER BY ts,ip LIMIT 4096")?;
        for row in discovered_stmt.query_map([&selected], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })? {
            let (ts, body) = row?;
            let mut found: DiscoveredDevice = serde_json::from_str(&body)?;
            found.details.hostname = found.details.hostname.or(found.hostname.clone());
            found.details.vendor = found.details.vendor.or(found.vendor.clone());
            found.details.observed_at.get_or_insert(ts);
            found
                .details
                .source
                .get_or_insert_with(|| "Nmap discovery".into());
            let id = identities.id(&found.ip, &found.mac);
            let name = identities
                .name(&found.ip, &found.mac, &names)
                .or_else(|| found.hostname.clone())
                .unwrap_or(found.ip.clone());
            let first = self
                .conn
                .query_row(
                    "SELECT ts FROM discovery_first WHERE sensor=?1 AND ip=?2 AND mac=?3",
                    params![
                        selected,
                        found.ip,
                        usable_mac(&found.mac).unwrap_or_default()
                    ],
                    |r| r.get(0),
                )
                .unwrap_or(ts);
            if let Some(existing) = devices.get_mut(&id) {
                existing.details = merge_details(existing.details.clone(), found.details);
                existing.addresses.push(found.ip);
                existing.first_seen = existing.first_seen.min(first);
                existing.last_seen = existing.last_seen.max(ts);
                if name != existing.addresses.last().cloned().unwrap_or_default() {
                    existing.name = name;
                }
                continue;
            }
            devices.insert(id.clone(),Device {details: found.details, id,name,addresses:vec![found.ip],mac:found.mac,category:"Unknown".into(),identification:format!("Nmap discovery; reported vendor: {}. Hostnames and vendors are hints, not verified identity.",found.vendor.unwrap_or("unknown".into())),first_seen:first,last_seen:ts,upload:0,download:0,local_bytes:0,connections:0});
        }
        // Correlate native host discovery with native host capture only on BOTH MAC and IP.
        // This enriches identity hints without combining traffic totals or collector domains.
        let mut host_hints = std::collections::HashMap::new();
        if selected
            .as_deref()
            .is_some_and(|id| id.starts_with("host-") && id != "host-discovery")
        {
            let mut stmt = self.conn.prepare(
                "SELECT ts,body FROM discovery WHERE sensor='host-discovery' LIMIT 4096",
            )?;
            for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
                let (ts, body) = row?;
                let mut found: DiscoveredDevice = serde_json::from_str(&body)?;
                found.details.hostname = found.details.hostname.or(found.hostname.clone());
                found.details.vendor = found.details.vendor.or(found.vendor.clone());
                found.details.observed_at.get_or_insert(ts);
                found.details.source = Some(format!(
                    "{}; matched MAC and IP",
                    found
                        .details
                        .source
                        .as_deref()
                        .unwrap_or("Local Nmap discovery")
                ));
                if let Some(mac) = &found.mac {
                    host_hints.insert((found.ip.clone(), mac.to_lowercase()), (ts, found));
                }
            }
        }
        let mut conversations: BTreeMap<String, Conversation> = BTreeMap::new();
        let mut timeline: BTreeMap<i64, Bucket> = BTreeMap::new();
        let span = observations
            .first()
            .zip(observations.last())
            .map(|(last, first)| last.timestamp - first.timestamp)
            .unwrap_or(0);
        let bucket_seconds = ((span / 240 + 299) / 300 * 300).max(300);
        let mut totals = Totals::default();
        let mut uploads: BTreeMap<(String, i64), (u64, Vec<String>, i64)> = BTreeMap::new();
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
            let src_id = src_local.then(|| identities.id(&o.src_ip, &o.src_mac));
            let dst_id = dst_local.then(|| identities.id(&o.dst_ip, &o.dst_mac));
            for (id, ip, mac, source) in [
                (&src_id, &o.src_ip, &o.src_mac, true),
                (&dst_id, &o.dst_ip, &o.dst_mac, false),
            ] {
                if let Some(id) = id {
                    let d = devices.entry(id.clone()).or_insert_with(|| Device {
                        details: DeviceDetails::default(),
                        id: id.clone(),
                        name: identities
                            .name(ip, mac, &names)
                            .unwrap_or_else(|| ip.clone()),
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
            if direction == "upload" {
                if let Some(id) = &c.src_device {
                    let entry = uploads
                        .entry((id.clone(), o.timestamp / 3600 * 3600))
                        .or_default();
                    entry.0 += o.bytes;
                    if entry.1.len() < 5 && !entry.1.contains(&c.id) {
                        entry.1.push(c.id.clone());
                    }
                    entry.2 = entry.2.max(o.timestamp);
                }
            }
            let bucket_ts = o.timestamp / bucket_seconds * bucket_seconds;
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
        let mut device_evidence: HashMap<String, Vec<String>> = HashMap::new();
        for c in conversations.values() {
            for id in [&c.src_device, &c.dst_device].into_iter().flatten() {
                if let Some(d) = devices.get_mut(id) {
                    d.connections += 1;
                    let evidence = device_evidence.entry(id.clone()).or_default();
                    if evidence.len() < 3 {
                        evidence.push(c.id.clone());
                    }
                }
            }
        }
        for d in devices.values_mut() {
            let Some(mac) = &d.mac else { continue };
            let Some((ts, found)) = d
                .addresses
                .iter()
                .find_map(|ip| host_hints.get(&(ip.clone(), mac.to_lowercase())))
            else {
                continue;
            };
            if d.last_seen.abs_diff(*ts) > 86400 {
                continue;
            }
            d.details = found.details.clone();
            if !names.contains_key(&d.id) {
                let discovery_id = device_id("host-discovery", &found.ip, &found.mac);
                let mac_id = format!("host-discovery:mac:{}", mac.to_lowercase());
                if let Some(name) = names
                    .get(&mac_id)
                    .or(names.get(&discovery_id))
                    .or(found.hostname.as_ref())
                {
                    d.name = name.clone();
                }
            }
            d.identification = "Observed traffic with local discovery hints matched by MAC and IP. Identity is not verified.".into();
        }
        for d in devices.values_mut() {
            let ip = d.addresses.first().cloned().unwrap_or_default();
            d.identification = identities.description(&ip, &d.mac);
            if let Some((first, last, addresses)) = identities.history(&ip, &d.mac) {
                d.first_seen = d.first_seen.min(first);
                d.last_seen = d.last_seen.max(last);
                d.addresses = addresses;
            }
        }
        let mut alerts = Vec::new();
        for d in devices.values() {
            let id = format!("device:{}", d.id);
            let was_reviewed = acknowledged.contains(&id)
                || d.addresses.iter().any(|ip| {
                    acknowledged.contains(&format!(
                        "device:{}",
                        device_id(selected.as_deref().unwrap_or(""), ip, &d.mac)
                    ))
                });
            alerts.push(Alert { acknowledged:was_reviewed, id,device_id:d.id.clone(),severity:"info".into(),title:"Device observed".into(),
                detail:"This device appears in retained observations. This does not establish when it joined your network or whether it is trusted.".into(),
                evidence:device_evidence.get(&d.id).cloned().unwrap_or_default(),timestamp:d.first_seen });
        }
        for ((device_id, hour), (bytes, evidence, timestamp)) in uploads {
            if bytes < 50 * 1024 * 1024 {
                continue;
            }
            let id = format!("upload:{device_id}:{hour}");
            alerts.push(Alert { acknowledged: acknowledged.contains(&id), id, device_id, severity:"notice".into(),title:"Large observed upload".into(),detail:format!("At least 50 MiB sent outside configured local networks in the UTC hour beginning {}. This view may cover only part of that hour. Backups and video calls can explain this; it is not a malware finding.", chrono::DateTime::from_timestamp(hour,0).map(|v|v.to_rfc3339()).unwrap_or_default()), evidence,timestamp });
        }
        alerts.sort_by_key(|a| std::cmp::Reverse(a.timestamp));
        let mut conversations: Vec<_> = conversations.into_values().collect();
        conversations.sort_by_key(|c| std::cmp::Reverse(c.bytes));
        Ok(Snapshot {
            bucket_seconds,
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
            limited: false,
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

#[derive(Default)]
struct Identities {
    sensor: String,
    endpoints: BTreeMap<String, Vec<(String, i64, i64)>>,
    ambiguous: HashSet<String>,
}
impl Identities {
    fn id(&self, ip: &str, mac: &Option<String>) -> String {
        match usable_mac(mac) {
            Some(mac) if !self.ambiguous.contains(&mac) => format!("{}:mac:{mac}", self.sensor),
            _ => device_id(&self.sensor, ip, mac),
        }
    }
    fn name(
        &self,
        ip: &str,
        mac: &Option<String>,
        names: &HashMap<String, String>,
    ) -> Option<String> {
        let id = self.id(ip, mac);
        names.get(&id).cloned().or_else(|| {
            let mac = usable_mac(mac)?;
            if self.ambiguous.contains(&mac) {
                return None;
            }
            // Retain names from pre-MAC-identity versions; conflicting names never migrate silently.
            self.endpoints.get(&mac)?.iter().find_map(|(ip, _, _)| {
                names
                    .get(&device_id(&self.sensor, ip, &Some(mac.clone())))
                    .cloned()
            })
        })
    }
    fn history(&self, ip: &str, mac: &Option<String>) -> Option<(i64, i64, Vec<String>)> {
        let mac = usable_mac(mac)?;
        let entries: Vec<_> = self
            .endpoints
            .get(&mac)?
            .iter()
            .filter(|(address, _, _)| !self.ambiguous.contains(&mac) || address == ip)
            .collect();
        Some((
            entries.iter().map(|e| e.1).min()?,
            entries.iter().map(|e| e.2).max()?,
            entries.iter().map(|e| e.0.clone()).collect(),
        ))
    }
    fn description(&self, _ip: &str, mac: &Option<String>) -> String {
        match usable_mac(mac) {
            Some(mac) if self.ambiguous.contains(&mac) => "Shared or ambiguous MAC: addresses overlap in time or have conflicting names. Kept separate; may be a gateway, proxy, or multiple interfaces.".into(),
            Some(_) => "MAC-linked identity within this observation source. Names follow address changes. MAC addresses can be randomized or spoofed; identity is not verified.".into(),
            None => "IP-only identity: no usable endpoint MAC observed. A reused IP may belong to another device.".into(),
        }
    }
}
impl Store {
    fn identities(
        &self,
        sensor: &str,
        nets: &[ipnet::IpNet],
        names: &HashMap<String, String>,
    ) -> Result<Identities> {
        let mut value = Identities {
            sensor: sensor.into(),
            ..Default::default()
        };
        let mut stmt = self.conn.prepare(
            "SELECT mac,ip,first,last FROM endpoint_identity WHERE sensor=?1 ORDER BY first,ip",
        )?;
        for row in stmt.query_map([sensor], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })? {
            let (mac, ip, first, last) = row?;
            if usable_mac(&Some(mac.clone())).is_some() && local(&ip, nets) && unicast(&ip) {
                value
                    .endpoints
                    .entry(mac)
                    .or_default()
                    .push((ip, first, last));
            }
        }
        for (mac, entries) in &value.endpoints {
            let labels: HashSet<_> = entries
                .iter()
                .filter_map(|(ip, _, _)| names.get(&device_id(sensor, ip, &Some(mac.clone()))))
                .collect();
            let overlaps = entries
                .windows(2)
                .any(|pair| pair[0].2.saturating_add(300) >= pair[1].1);
            if overlaps || labels.len() > 1 {
                value.ambiguous.insert(mac.clone());
            }
        }
        Ok(value)
    }
    pub fn storage_limit(&self) -> Result<usize> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='event_limit'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(value.and_then(|v| v.parse().ok()).unwrap_or(MAX_EVENTS))
    }
    pub fn set_storage_limit(&self, limit: usize) -> Result<()> {
        if !(1_000..=MAX_EVENTS).contains(&limit) {
            bail!("Keep between 1,000 and 100,000 observations");
        }
        // A lower limit takes effect on the next ingest. The UI explains this before saving.
        self.conn.execute(
            "INSERT OR REPLACE INTO settings VALUES ('event_limit',?1)",
            [limit.to_string()],
        )?;
        Ok(())
    }
    pub fn export_local(&self) -> Result<serde_json::Value> {
        let mut tables = serde_json::Map::new();
        for table in [
            "observations",
            "sensors",
            "discovery",
            "explanation_history",
        ] {
            let mut stmt = self.conn.prepare(&format!("SELECT body FROM {table}"))?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))?
                .map(|r| Ok(serde_json::from_str::<serde_json::Value>(&r?)?))
                .collect::<Result<Vec<_>>>()?;
            tables.insert(table.into(), serde_json::json!(rows));
        }
        let mut stmt = self.conn.prepare("SELECT id,name FROM names")?;
        let names = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<BTreeMap<_, _>, _>>()?;
        tables.insert("names".into(), serde_json::json!(names));
        tables.insert("formatVersion".into(), serde_json::json!(1));
        Ok(tables.into())
    }
    pub fn clear_local_data(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        for table in [
            "observations",
            "sensors",
            "discovery",
            "discovery_first",
            "endpoint_identity",
            "names",
            "acknowledged",
            "explanation_history",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")?;
        Ok(())
    }
}

impl Store {
    pub fn save_explanation(&self, body: &serde_json::Value) -> Result<()> {
        let text = serde_json::to_string(body)?;
        if text.len() > 512 * 1024 {
            bail!("Explanation exceeds the local history limit");
        }
        let id = format!("{:x}", Sha256::digest(text.as_bytes()));
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO explanation_history VALUES (?1,?2,?3)",
            params![id, chrono::Utc::now().timestamp(), text],
        )?;
        tx.execute("DELETE FROM explanation_history WHERE rowid IN (SELECT rowid FROM explanation_history ORDER BY saved DESC,rowid DESC LIMIT -1 OFFSET 20)",[])?;
        tx.commit()?;
        Ok(())
    }
    pub fn explanation_history(&self) -> Result<Vec<serde_json::Value>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,saved,body FROM explanation_history ORDER BY saved DESC,rowid DESC",
        )?;
        let rows = stmt.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?)))?.map(|r| {
            let(id,saved,body)=r?;
            Ok(serde_json::json!({"id":id,"savedAt":saved,"body":serde_json::from_str::<serde_json::Value>(&body)?}))
        }).collect();
        rows
    }
    pub fn delete_explanation(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM explanation_history WHERE id=?1", [id])?;
        Ok(())
    }
}
