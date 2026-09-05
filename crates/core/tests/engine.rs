use hns_core::*;

fn store() -> Store {
    let s = Store::memory().unwrap();
    s.set_sensor(&Sensor::new("test", "fixture")).unwrap();
    s
}
fn event(id: &str, src: &str, dst: &str, bytes: u64) -> Observation {
    Observation {
        id: id.into(),
        sensor_id: "test".into(),
        timestamp: 1_788_470_400,
        src_ip: src.into(),
        dst_ip: dst.into(),
        src_mac: None,
        dst_mac: None,
        src_port: Some(40000),
        dst_port: Some(443),
        protocol: "TLS".into(),
        bytes,
        packets: 1,
    }
}

#[test]
fn accounts_wan_and_lan_separately() {
    let mut s = store();
    s.ingest(&[
        event("1", "10.0.0.2", "203.0.113.1", 100),
        event("2", "203.0.113.1", "10.0.0.2", 300),
        event("3", "10.0.0.2", "10.0.0.3", 500),
    ])
    .unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(
        (
            view.totals.upload,
            view.totals.download,
            view.totals.local_bytes
        ),
        (100, 300, 500)
    );
    assert_eq!(view.devices.len(), 2);
    assert_eq!(
        view.devices.iter().map(|d| d.local_bytes).sum::<u64>(),
        1000
    );
}
#[test]
fn duplicate_import_does_not_inflate_traffic() {
    let mut s = store();
    let events = [event("1", "10.0.0.2", "203.0.113.1", 123)];
    assert_eq!(s.ingest(&events).unwrap(), 1);
    assert_eq!(s.ingest(&events).unwrap(), 0);
    assert_eq!(s.snapshot(None, "local").unwrap().totals.upload, 123);
}
#[test]
fn rejects_invalid_batch_atomically() {
    let mut s = store();
    let invalid = event("2", "not-an-ip", "10.0.0.1", 500);
    assert!(s
        .ingest(&[event("1", "10.0.0.2", "203.0.113.1", 10), invalid])
        .is_err());
    assert_eq!(s.snapshot(None, "local").unwrap().observation_count, 0);
}
#[test]
fn unknown_sensor_is_not_silently_accepted() {
    let mut s = Store::memory().unwrap();
    assert!(s
        .ingest(&[event("1", "10.0.0.2", "203.0.113.1", 1)])
        .is_err());
}
#[test]
fn multicast_is_not_internet_upload() {
    let mut s = store();
    s.ingest(&[event("1", "10.0.0.2", "224.0.0.251", 100)])
        .unwrap();
    let v = s.snapshot(None, "local").unwrap();
    assert_eq!(v.totals.upload, 0);
    assert_eq!(v.devices.len(), 1);
    assert_eq!(v.conversations[0].direction, "multicast");
}
#[test]
fn globally_routed_ipv6_can_be_local() {
    let mut s = store();
    s.set_networks("2001:db8:1::/64").unwrap();
    s.ingest(&[
        event("1", "2001:db8:1::1", "2001:db8:1::2", 100),
        event("2", "2001:db8:1::2", "2001:db8:2::1", 200),
    ])
    .unwrap();
    let v = s.snapshot(None, "local").unwrap();
    assert_eq!(v.totals.local_bytes, 100);
    assert_eq!(v.totals.upload, 200);
}
#[test]
fn renames_and_review_status_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("network.db");
    let mut s = Store::open(&path).unwrap();
    s.set_sensor(&Sensor::new("test", "file")).unwrap();
    s.ingest(&[event("1", "10.0.0.2", "203.0.113.1", 100)])
        .unwrap();
    let v = s.snapshot(None, "local").unwrap();
    let id = v.devices[0].id.clone();
    let alert = v.alerts[0].id.clone();
    s.rename(&id, "My laptop").unwrap();
    s.acknowledge(&alert).unwrap();
    drop(s);
    let s = Store::open(&path).unwrap();
    let v = s.snapshot(None, "local").unwrap();
    assert_eq!(v.devices[0].name, "My laptop");
    assert!(v.alerts[0].acknowledged);
}
#[test]
fn sensors_are_not_summed_or_merged() {
    let mut s = store();
    s.set_sensor(&Sensor::new("other", "file")).unwrap();
    let a = event("1", "10.0.0.2", "203.0.113.1", 100);
    let mut b = a.clone();
    b.sensor_id = "other".into();
    s.ingest(&[a, b]).unwrap();
    assert_eq!(
        s.snapshot(Some("test"), "local").unwrap().totals.upload,
        100
    );
    assert_eq!(
        s.snapshot(Some("other"), "local").unwrap().totals.upload,
        100
    );
    assert!(s.snapshot(Some("missing"), "local").is_err());
}
#[test]
fn routed_devices_with_shared_gateway_mac_are_distinct() {
    let mut s = store();
    let mut a = event("1", "10.0.0.2", "203.0.113.1", 100);
    a.src_mac = Some("02:00:00:00:00:01".into());
    let mut b = a.clone();
    b.id = "2".into();
    b.src_ip = "10.0.1.2".into();
    s.ingest(&[a, b]).unwrap();
    assert_eq!(s.snapshot(None, "local").unwrap().devices.len(), 2);
}
#[test]
fn large_upload_alert_has_real_evidence_and_qualified_language() {
    let mut s = store();
    s.ingest(&[event("1", "10.0.0.2", "203.0.113.1", 60 * 1024 * 1024)])
        .unwrap();
    let v = s.snapshot(None, "local").unwrap();
    let alert = v.alerts.iter().find(|a| a.severity == "notice").unwrap();
    assert!(alert.detail.contains("not a malware finding"));
    assert!(alert
        .evidence
        .iter()
        .all(|id| v.conversations.iter().any(|c| &c.id == id)));
}
#[test]
fn discovery_does_not_invent_traffic() {
    let s = store();
    s.save_discovery(
        "test",
        &[DiscoveredDevice {
            details: DeviceDetails::default(),
            ip: "10.0.0.5".into(),
            mac: None,
            hostname: Some("printer.local".into()),
            vendor: None,
        }],
    )
    .unwrap();
    let v = s.snapshot(None, "local").unwrap();
    assert_eq!(v.devices[0].name, "printer.local");
    assert_eq!(v.totals.packets, 0);
    assert_eq!(v.observation_count, 0);
}
#[test]
fn demo_is_synthetic_and_isolated() {
    let demo = demo_store().unwrap();
    let live = Store::memory().unwrap();
    let v = demo.snapshot(None, "demo").unwrap();
    assert_eq!(v.devices.len(), 6);
    assert!(v.totals.local_bytes > 0);
    assert!(v.totals.upload > 0);
    assert!(v.totals.download > 0);
    assert!(live.snapshot(None, "local").unwrap().devices.is_empty());
    assert!(v
        .sensors
        .iter()
        .all(|s| s.internet_coverage == "unverified" && s.lan_coverage == "unverified"));
}

#[test]
fn host_discovery_hints_require_matching_mac_ip_and_domain() {
    let mut s = Store::memory().unwrap();
    for id in ["host-discovery", "host-fixture", "remote-fixture"] {
        s.set_sensor(&Sensor::new(id, "tshark")).unwrap();
    }
    let now = chrono::Utc::now().timestamp();
    s.save_discovery(
        "host-discovery",
        &[DiscoveredDevice {
            ip: "10.0.0.2".into(),
            mac: Some("02:00:00:00:00:02".into()),
            hostname: Some("fixture.local".into()),
            vendor: None,
            details: DeviceDetails::default(),
        }],
    )
    .unwrap();
    let mut e = event("matched", "10.0.0.2", "203.0.113.1", 100);
    e.timestamp = now;
    e.sensor_id = "host-fixture".into();
    e.src_mac = Some("02:00:00:00:00:02".into());
    s.ingest(&[e.clone()]).unwrap();
    let v = s.snapshot(Some("host-fixture"), "local").unwrap();
    assert_eq!(v.devices[0].name, "fixture.local");
    assert_eq!(v.totals.packets, 1);
    e.sensor_id = "remote-fixture".into();
    s.ingest(&[e.clone()]).unwrap();
    assert!(
        s.snapshot(Some("remote-fixture"), "local").unwrap().devices[0]
            .details
            .hostname
            .is_none()
    );
    e.sensor_id = "host-fixture".into();
    e.id = "same-mac-other-ip".into();
    e.src_ip = "10.0.0.3".into();
    s.ingest(&[e]).unwrap();
    let v = s.snapshot(Some("host-fixture"), "local").unwrap();
    let other = v
        .devices
        .iter()
        .find(|d| d.addresses.contains(&"10.0.0.3".into()))
        .unwrap();
    assert!(other.details.hostname.is_none());
    assert_eq!(v.totals.packets, 2);
}

#[test]
fn mac_name_follows_dhcp_and_preserves_addresses_without_cross_sensor_merge() {
    let mut s = store();
    let mut first = event("before", "10.0.0.2", "203.0.113.1", 100);
    first.src_mac = Some("02:AA:00:00:00:01".into());
    s.ingest(std::slice::from_ref(&first)).unwrap();
    let id = s.snapshot(None, "local").unwrap().devices[0].id.clone();
    s.rename(&id, "Office laptop").unwrap();
    let mut later = first.clone();
    later.id = "after".into();
    later.src_ip = "10.0.0.7".into();
    later.src_mac = Some("02:aa:00:00:00:01".into());
    later.timestamp += 3600;
    s.ingest(&[later.clone()]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(view.devices.len(), 1);
    assert_eq!(view.devices[0].name, "Office laptop");
    assert_eq!(view.devices[0].id, id);
    assert_eq!(view.devices[0].addresses.len(), 2);
    assert_eq!(view.devices[0].upload, 200);
    s.set_sensor(&Sensor::new("other", "file")).unwrap();
    later.sensor_id = "other".into();
    s.ingest(&[later]).unwrap();
    assert_ne!(
        s.snapshot(Some("other"), "local").unwrap().devices[0].name,
        "Office laptop"
    );
}
#[test]
fn shared_mac_does_not_propagate_a_name_or_merge_devices() {
    let mut s = store();
    let mut a = event("a", "10.0.0.2", "203.0.113.1", 100);
    a.src_mac = Some("02:00:00:00:00:01".into());
    let mut b = a.clone();
    b.id = "b".into();
    b.src_ip = "10.0.0.3".into();
    s.ingest(&[a, b]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    s.rename(&view.devices[0].id, "Only this address").unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(view.devices.len(), 2);
    assert_eq!(
        view.devices
            .iter()
            .filter(|d| d.name == "Only this address")
            .count(),
        1
    );
    assert!(view
        .devices
        .iter()
        .all(|d| d.identification.contains("ambiguous")));
}
#[test]
fn old_address_names_migrate_to_mac_identity_and_conflicts_stay_separate() {
    let mut s = store();
    let mut e = event("a", "10.0.0.2", "203.0.113.1", 10);
    e.src_mac = Some("02:00:00:00:00:01".into());
    s.rename(&device_id("test", &e.src_ip, &e.src_mac), "Old label")
        .unwrap();
    s.ingest(&[e.clone()]).unwrap();
    assert_eq!(
        s.snapshot(None, "local").unwrap().devices[0].name,
        "Old label"
    );
    e.id = "b".into();
    e.timestamp += 3600;
    e.src_ip = "10.0.0.9".into();
    s.ingest(&[e.clone()]).unwrap();
    assert_eq!(
        s.snapshot(None, "local").unwrap().devices[0].name,
        "Old label"
    );
    s.rename(
        &device_id("test", &e.src_ip, &e.src_mac),
        "Conflicting label",
    )
    .unwrap();
    assert_eq!(s.snapshot(None, "local").unwrap().devices.len(), 2);
}
#[test]
fn rediscovery_preserves_services_and_first_seen_but_not_across_mac_reuse() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.db");
    let s = Store::open(&path).unwrap();
    s.set_sensor(&Sensor::new("test", "nmap")).unwrap();
    let found = DiscoveredDevice {
        ip: "10.0.0.2".into(),
        mac: Some("02:00:00:00:00:01".into()),
        hostname: None,
        vendor: None,
        details: DeviceDetails {
            operating_system: Some("Observed OS guess".into()),
            services: vec![ObservedService {
                observed_at: None,
                port: 22,
                transport: "tcp".into(),
                name: Some("ssh".into()),
                product: None,
                version: None,
            }],
            ..Default::default()
        },
    };
    s.save_discovery("test", std::slice::from_ref(&found))
        .unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("UPDATE discovery_first SET ts=100", [])
        .unwrap();
    let mut basic = found.clone();
    basic.details = DeviceDetails::default();
    s.save_discovery("test", &[basic.clone()]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(view.devices[0].first_seen, 100);
    assert_eq!(view.devices[0].details.services.len(), 1);
    assert!(view.devices[0].details.operating_system.is_some());
    let observed = view.devices[0].details.services[0].observed_at;
    assert!(observed.is_some());
    basic.mac = Some("02:00:00:00:00:02".into());
    s.save_discovery("test", &[basic]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert!(view.devices[0].details.services.is_empty());
    assert!(view.devices[0].first_seen > 100);
}
#[test]
fn acknowledging_one_upload_hour_does_not_acknowledge_the_next() {
    let mut s = store();
    let first = event("a", "10.0.0.2", "203.0.113.1", 60 * 1024 * 1024);
    s.ingest(std::slice::from_ref(&first)).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    let id = view
        .alerts
        .iter()
        .find(|a| a.severity == "notice")
        .unwrap()
        .id
        .clone();
    s.acknowledge(&id).unwrap();
    let mut next = first;
    next.id = "b".into();
    next.timestamp += 3600;
    s.ingest(&[next]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    let alerts: Vec<_> = view
        .alerts
        .iter()
        .filter(|a| a.severity == "notice")
        .collect();
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts.iter().filter(|a| a.acknowledged).count(), 1);
    assert!(alerts.iter().any(|a| a.id != id && !a.acknowledged));
}
#[test]
fn time_ranges_use_all_retained_events_and_keep_identity_history() {
    let mut s = store();
    let mut events = Vec::new();
    for i in 0..10001 {
        let mut e = event(&format!("a{i}"), "10.0.0.2", "203.0.113.1", 1);
        e.timestamp += i;
        e.src_mac = Some("02:00:00:00:00:01".into());
        events.push(e);
    }
    s.ingest(&events).unwrap();
    let all = s.snapshot(None, "local").unwrap();
    assert_eq!(all.totals.upload, 10001);
    let view = s
        .snapshot_since(None, "local", Some(events[10000].timestamp))
        .unwrap();
    assert_eq!(view.totals.upload, 1);
    assert_eq!(view.devices[0].first_seen, events[0].timestamp);
    s.set_storage_limit(1000).unwrap();
    let mut next = events[10000].clone();
    next.id = "last".into();
    next.timestamp += 1;
    s.ingest(&[next]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(view.retained_count, 1000);
    assert_eq!(view.devices[0].first_seen, events[0].timestamp);
}
#[test]
fn export_and_delete_include_saved_explanations_without_credentials() {
    let mut s = store();
    s.ingest(&[event("a", "10.0.0.2", "203.0.113.1", 1)])
        .unwrap();
    for i in 0..21 {
        s.save_explanation(
            &serde_json::json!({"text":format!("synthetic {i}"),"summary":"fixture"}),
        )
        .unwrap();
    }
    assert_eq!(s.explanation_history().unwrap().len(), 20);
    let value = s.export_local().unwrap();
    assert_eq!(value["observations"].as_array().unwrap().len(), 1);
    assert_eq!(value["explanation_history"].as_array().unwrap().len(), 20);
    assert!(value.get("providers").is_none());
    s.clear_local_data().unwrap();
    assert!(s.explanation_history().unwrap().is_empty());
    assert!(s.sensors().unwrap().is_empty());
    assert_eq!(s.storage_limit().unwrap(), 100000);
}

#[test]
fn a_later_shared_mac_keeps_the_original_name_on_its_original_address() {
    let mut s = store();
    let mut a = event("a", "10.0.0.2", "203.0.113.1", 10);
    a.src_mac = Some("02:00:00:00:00:01".into());
    s.ingest(std::slice::from_ref(&a)).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    s.rename(&view.devices[0].id, "Original device").unwrap();
    s.acknowledge(&format!(
        "device:{}",
        device_id("test", &a.src_ip, &a.src_mac)
    ))
    .unwrap();
    assert!(s.snapshot(None, "local").unwrap().alerts[0].acknowledged);
    let mut b = a;
    b.id = "b".into();
    b.src_ip = "10.0.0.3".into();
    s.ingest(&[b]).unwrap();
    let view = s.snapshot(None, "local").unwrap();
    assert_eq!(view.devices.len(), 2);
    assert_eq!(
        view.devices
            .iter()
            .find(|d| d.addresses[0] == "10.0.0.2")
            .unwrap()
            .name,
        "Original device"
    );
    assert_ne!(
        view.devices
            .iter()
            .find(|d| d.addresses[0] == "10.0.0.3")
            .unwrap()
            .name,
        "Original device"
    );
}
