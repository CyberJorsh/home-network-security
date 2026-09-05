//! OS-reported identity for this computer only; never inferred for neighboring hosts.
use hns_core::{bounded_output_cancellable, tool_command, DiscoveredDevice};
use serde::Deserialize;
use std::{sync::atomic::AtomicBool, time::Duration};

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Identity {
    name: Option<String>,
    model: Option<String>,
    vendor: Option<String>,
    os: Option<String>,
    #[serde(default)]
    adapters: Vec<Adapter>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Adapter {
    addresses: Vec<String>,
    mac: Option<String>,
}
fn text(command: &str, args: &[&str], cancel: &AtomicBool) -> Option<String> {
    let bytes = bounded_output_cancellable(
        tool_command(command).args(args),
        65536,
        Duration::from_secs(10),
        cancel,
    )
    .ok()?;
    let value = String::from_utf8(bytes).ok()?.trim().to_string();
    (!value.is_empty()).then_some(value)
}
fn read(cancel: &AtomicBool) -> Identity {
    if cfg!(target_os = "macos") {
        let adapters = if_addrs::get_if_addrs()
            .unwrap_or_default()
            .into_iter()
            .filter(|i| !i.is_loopback())
            .filter_map(|i| {
                let output = text("/sbin/ifconfig", &[&i.name], cancel)?;
                let mac = output.lines().find_map(|l| {
                    l.trim()
                        .strip_prefix("ether ")
                        .and_then(|v| v.split_whitespace().next())
                        .map(str::to_owned)
                });
                Some(Adapter {
                    addresses: vec![i.ip().to_string()],
                    mac,
                })
            })
            .collect();
        Identity {
            name: text("/bin/hostname", &[], cancel),
            model: text("/usr/sbin/sysctl", &["-n", "hw.model"], cancel),
            vendor: Some("Apple".into()),
            os: text("/usr/bin/sw_vers", &["-productVersion"], cancel)
                .map(|v| format!("macOS {v} (reported by this computer)")),
            adapters,
        }
    } else if cfg!(windows) {
        // Fixed read-only OS inventory. No device address is interpolated into PowerShell.
        let script = "$ErrorActionPreference='Stop'; $c=Get-CimInstance Win32_ComputerSystem; $o=Get-CimInstance Win32_OperatingSystem; $a=@(Get-CimInstance Win32_NetworkAdapterConfiguration -Filter 'IPEnabled = True' | ForEach-Object { @{addresses=@($_.IPAddress); mac=$_.MACAddress} }); @{name=$env:COMPUTERNAME; model=$c.Model; vendor=$c.Manufacturer; os=($o.Caption+' '+$o.Version+' (reported by this computer)'); adapters=$a} | ConvertTo-Json -Depth 4 -Compress";
        text(
            "powershell.exe",
            &["-NoProfile", "-Command", script],
            cancel,
        )
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_default()
    } else {
        Identity::default()
    }
}
fn apply(found: &mut [DiscoveredDevice], identity: Identity) {
    for device in found {
        let Some(adapter) = identity
            .adapters
            .iter()
            .find(|a| a.addresses.contains(&device.ip))
        else {
            continue;
        };
        device.hostname = identity.name.clone().or(device.hostname.take());
        device.vendor = identity.vendor.clone().or(device.vendor.take());
        device.mac = adapter
            .mac
            .as_ref()
            .and_then(|m| {
                let m = m.replace('-', ":").to_lowercase();
                (m.len() == 17
                    && m.split(':').count() == 6
                    && m.split(':')
                        .all(|b| b.len() == 2 && u8::from_str_radix(b, 16).is_ok()))
                .then_some(m)
            })
            .or(device.mac.take());
        device.details.hostname = device.hostname.clone();
        device.details.vendor = device.vendor.clone();
        device.details.model = identity.model.clone().or(device.details.model.take());
        device.details.operating_system = identity
            .os
            .clone()
            .or(device.details.operating_system.take());
        device.details.source =
            Some("This computer's OS identity and Nmap service discovery".into());
    }
}
pub fn enrich(found: &mut [DiscoveredDevice], cancel: &AtomicBool) {
    if !found.is_empty() {
        apply(found, read(cancel));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn os_identity_only_enriches_an_address_of_this_computer() {
        let mut devices = hns_core::parse_nmap(r#"<nmaprun><host><status state="up"/><address addr="10.0.0.2" addrtype="ipv4"/></host><host><status state="up"/><address addr="10.0.0.3" addrtype="ipv4"/></host></nmaprun>"#).unwrap();
        let identity: Identity = serde_json::from_value(serde_json::json!({"name":"Fixture PC","model":"Fixture model","vendor":"Fixture vendor","os":"Fixture OS","adapters":[{"addresses":["10.0.0.2"],"mac":"02-00-00-00-00-02"}]})).unwrap();
        apply(&mut devices, identity);
        assert_eq!(devices[0].hostname.as_deref(), Some("Fixture PC"));
        assert_eq!(devices[0].mac.as_deref(), Some("02:00:00:00:00:02"));
        assert_eq!(devices[0].details.model.as_deref(), Some("Fixture model"));
        assert!(devices[1].hostname.is_none() && devices[1].details.model.is_none());
    }
}
