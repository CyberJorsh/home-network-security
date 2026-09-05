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
