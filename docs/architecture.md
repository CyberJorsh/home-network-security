# Architecture

```text
Nmap XML ───────────────→ discovery records ─┐
TShark / NDJSON ────────→ validated events ──┼─→ SQLite → snapshot → React desktop UI
                                           │               ↑
                                   Rust collector ─ loopback API / SSH tunnel
                                                           │
                                             Tauri native command boundary
```

`crates/core` owns data validation, IP-prefix classification, SQLite persistence, parser adapters, snapshots, and deterministic alerts. `crates/collector` owns explicit CLI capture/discovery and the authenticated loopback API. `src-tauri` exposes a narrow native interface to a React/TypeScript webview. `src` contains the UI; the browser preview loads only the committed synthetic JSON snapshot.

Every observation identifies a sensor. A snapshot selects exactly one sensor to avoid summing overlapping capture points. Discovery records do not invent packet counts. Device identities use sensor, observed MAC when available, and IP; this avoids merging routed endpoints behind a gateway MAC but intentionally does not claim stable household identity across DHCP or IPv6 address changes.

A conversation groups source/destination IPs and ports plus protocol within a sensor. Accounting classifies local→external as upload, external→local as download, local→local as local traffic, multicast separately, and other traffic as transit. Local bytes count once in the network total and once for each participating endpoint's attribution.

Alerts currently report newly observed devices in the retained view and device uploads of at least 50 MiB in that view. Both are observations, not threat classifications. Evidence references point to conversations present in the same snapshot. Acknowledgement survives application restarts; eviction changes the retained observation window.

The native app can read its own database or one authenticated collector. Collector credentials are not passed to the browser renderer's general networking APIs. There is no cloud backend or mandatory account. Optional AI currently ends at an editable, redacted summary and an explicit copy action; embedded providers remain a separate acceptance gate.
