# Collector setup and coverage

The collector runs as a foreground process, not an installed service. Build with `cargo build --release -p hns-collector`. Run it as your normal user. Install current TShark/Wireshark and optionally Nmap using their official platform instructions. Do not run the entire desktop application with elevated privileges.

## Choose an observation point

| Placement                                     | Potential visibility                          | What can remain invisible                                                 |
| --------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------------------- |
| Ordinary Mac, PC, or Raspberry Pi switch port | Its own traffic, some broadcast/multicast     | Most unicast traffic between other devices                                |
| Router LAN/WAN observation point              | Traffic traversing that interface             | Same-switch and same-AP local transfers; pre-NAT identity on WAN          |
| Managed switch mirror/SPAN or tap             | Selected mirrored links/VLANs                 | Unmirrored ports, oversubscribed mirror drops, hardware-offloaded traffic |
| AP/router export                              | Whatever its supported export includes        | Export omissions and aggregation; adapter work is still required          |
| ESXi/other virtual-switch observation point   | Explicitly permitted mirrored virtual traffic | Other physical switches, VLANs, and virtual port groups                   |

A Raspberry Pi is an optional collector host, not a visibility shortcut. ESXi configuration is environment-specific and must be verified against its virtual-switch/port-group policy. This app does not configure routers, switches, hypervisors, or mirrors for you.

Test both an internet transfer and a transfer between two other home devices at the intended observation point. Compare recorded endpoints and byte counts with your test traffic, and inspect capture loss. Start coverage unverified. This alpha exposes unverified coverage and unknown loss; it does not yet store a verified topology or packet-drop telemetry.

## Local capture and discovery

```sh
tshark -D
./target/release/hns-collector --sensor mirror capture --interface YOUR_INTERFACE --seconds 60
./target/release/hns-collector --sensor mirror discover 192.168.1.0/24
./target/release/hns-collector --sensor mirror snapshot
```

Discovery sends host-detection probes; it does not run port scans, exploit checks, or vulnerability scripts. Only select a network you are authorized to inspect. There is no scan-on-launch behavior. The GUI can import Nmap XML but does not yet launch active discovery or capture itself.

`capture` runs TShark with name resolution disabled, only emitting selected fields. No payload file is created by this application. It stops after the selected duration (1 second–24 hours), with a deadline and bounded queue. Exit diagnostics matter; packet loss is unknown. Sudden termination may leave the stored status as collecting; the GUI labels missing recent observations and does not call that a healthy sensor.

## Remote collector

Use a dedicated normal-user directory on your collector host. Keep the same `--db` path in the capture and server processes.

```sh
./hns-collector --db /your/private/path/network.db --sensor mirror capture --interface YOUR_INTERFACE --seconds 3600
# Second terminal; does not initiate capture:
./hns-collector --db /your/private/path/network.db serve --token-file /your/private/path/collector.token
# Desktop terminal:
ssh -N -L 9898:127.0.0.1:9898 user@collector
```

Read the token file locally through your trusted SSH session. In **Collection**, enter port 9898 and the token. The token stays in native app memory for the session. Protect it like a password. Keep your SSH connection open. API bind address cannot be changed to a public interface.

A local file import switches the desktop back to its own database and disconnects the remote session. Choose the observation source in the top bar when multiple sources exist. Sources remain separate; selecting one does not add overlapping packets from another.

## Prefixes and formats

Default local prefixes: `10.0.0.0/8,172.16.0.0/12,192.168.0.0/16,fc00::/7`. Customize via **Collection → Your local networks** for the desktop database, or on the collector:

```sh
./hns-collector networks '192.168.1.0/24,fd00::/64,2001:db8:1234::/64'
```

The IPv6 example is documentation-only; use your actual delegated prefix. Prefixes are global to that database. Multicast/broadcast is separate from internet traffic. Byte counts are observed frame lengths, including headers and retransmissions, not application goodput or billing usage.

NDJSON contains one validated observation per line. See `fixtures/sample.ndjson` and `crates/core/src/model.rs`. Imports replace `sensorId` with the explicitly selected import sensor. `(sensorId,id)` is idempotent while retained; replaying records evicted by retention can reinsert them. PCAP IDs use a capture hash and frame number; separately captured copies of the same packet are not deduplicated automatically.

PCAP input is capped at 256 MiB, decoded output at 64 MiB, and 100,000 IP frames per import. Import timeout is 120 seconds. NDJSON input is capped at 32 MiB. Nmap XML is capped at 16 MiB and 4,096 returned addresses; external/internal entity declarations are rejected, while Nmap's bare standard doctype is accepted. Live Nmap discovery has a five-minute deadline. Split large files before importing; rejected imports do not partially add observations.

## Storage

CLI default: `.data/network.db` in your working directory. Desktop: Tauri's per-user local application-data directory under identifier `io.github.cyberjorsh.home-network-security`. On macOS this is normally `~/Library/Application Support/io.github.cyberjorsh.home-network-security`; on Windows it is normally under `%LOCALAPPDATA%`. Sample mode is a separate in-memory database.

The desktop polls every ten seconds. The latest 100,000 normalized observations are retained, with 10,000 per selected view. Device notes and review state persist separately. No archival rollups, wall-clock retention controls, or secure erase workflow are implemented yet.

## Desktop collection controls

Open **Collection → Scan from this computer**. The app enumerates actual TShark interfaces and local IP addresses, offers private IPv4 ranges of at most /24, and checks for Nmap. Choose the subnet before starting discovery; choose a concrete local interface and duration before capture. Both jobs run on workers while the UI continues reading SQLite. **Stop collection** cancels the active job, and closing the desktop stops its child processes. A capture can also stop automatically after its selected duration.

Discovery and each capture interface get separate observation sources. Starting a job switches the dashboard to the local database and selects that source; it disconnects a remote collector connection. No raw capture file is created by the application. Live metadata and discovery records are private local data and must not be attached to public issues.

On macOS, `brew install nmap wireshark` installs the CLI tools. Wireshark's signed ChmodBPF installer enables BPF access for members of `access_bpf`; Homebrew also exposes it as `brew install --cask wireshark-chmodbpf`. Administrator authorization is required. Restart the app after installation; depending on your environment, logging out or rebooting may be needed for group membership. On Windows install the Npcap driver with the appropriate license and permissions. Interface enumeration alone does not establish that a capture will succeed.

Unprivileged discovery can miss devices that do not respond to its probes. Host capture typically sees the computer's own traffic plus broadcasts and multicast, not all conversations between other devices. Coverage remains unverified and packet drops are shown as unknown. Add your actual globally routed local IPv6 prefix in **Your local networks** for correct direction classification.
