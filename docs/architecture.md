# Architecture

```text
Nmap XML ───────────────→ discovery records ─┐
TShark / NDJSON ────────→ validated events ──┼─→ SQLite → snapshot → React desktop UI
                                           │               ↑
                                   Rust collector ─ loopback API / SSH tunnel
                                                           │
                                             Tauri native command boundary
```

`crates/core` owns data validation, IP-prefix classification, SQLite persistence, parser adapters, cancellable capture, snapshots, and deterministic alerts. `crates/collector` owns explicit CLI capture/discovery and the authenticated loopback API. `src-tauri` exposes a narrow native interface to a React/TypeScript webview. `src` contains the UI; the browser preview loads only the committed synthetic JSON snapshot.

Every observation identifies a sensor. A snapshot selects exactly one sensor to avoid summing overlapping capture points. Discovery records do not invent packet counts. Device identities prefer a usable unicast MAC within a sensor. Names and address history follow non-overlapping address changes. If observed address lifetimes overlap (with a five-minute ambiguity margin), or existing address names conflict, the MAC is treated as shared and records remain address-specific. IP is the fallback when no usable MAC exists. MACs are evidence, not verified ownership; randomized MACs and cross-sensor reconciliation remain explicit limitations. Existing names and device acknowledgements are read through legacy address aliases.

A conversation groups source/destination IPs and ports plus protocol within a sensor. Accounting classifies local→external as upload, external→local as download, local→local as local traffic, multicast separately, and other traffic as transit. Local bytes count once in the network total and once for each participating endpoint's attribution.

Alerts currently report newly observed devices in the retained view and device uploads of at least 50 MiB within a UTC hour in that view. Both are observations, not threat classifications. Evidence references point to conversations present in the same snapshot. Acknowledgement survives application restarts and is scoped to each upload hour. Eviction changes the retained traffic window; persistent identity first-seen time is separate from traffic totals.

The native app can read its own database or one authenticated collector. Collector credentials are not passed to the browser renderer's general networking APIs. There is no cloud backend or mandatory account. Optional AI has isolated official-client authentication. Reviewed explanations use a single native worker, live model/effort catalogs, subscription usage preflight, restricted disposable client profiles, and bounded streamed text polled by the UI. Account operations are serialized during each request; see providers.md. Native collection runs on workers using separate SQLite connections so dashboard reads continue during capture; closing the desktop stops collection and login jobs.

Native host capture can display recent discovery hints when both MAC and IP match a local Nmap observation within 24 hours. This never combines traffic totals or merges external collector domains. Discovery source/time remain attached to those hints. Nmap OS guesses are labeled, and a traffic port does not establish a listening service.

Rediscovery merges positive identity/service evidence for the same MAC and address. Individual service and identity-field timestamps prevent an old observation from being presented as newly measured. A basic scan does not prove an old service closed; a new MAC at a reused address does not inherit its predecessor's details.
