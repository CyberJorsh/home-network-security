# Verification

## Automated checks

- Rust unit/integration tests cover internet/local accounting, IPv6 prefixes, multicast, duplicate IDs, atomic invalid imports, sensor separation, gateway-MAC ambiguity, persistent names/reviews, discovery without invented traffic, and evidence-backed upload alerts.
- Frontend unit tests cover scoped summaries, redaction, and conversation filtering.
- `scripts/smoke_collector.py` launches an isolated synthetic collector and tests actual HTTP authentication, snapshots, rename/review behavior, malformed requests, and error handling. It neither scans nor captures a network.
- `scripts/smoke_pcap.py` generates a four-packet synthetic Ethernet PCAP and exercises the installed TShark import end to end. It checks actual frame-byte totals and idempotent import. It never reads a live interface.
- CI checks formatting, types, tests, production web builds, Rust warnings, and macOS/Windows development bundles. See each Actions run for actual completion.

## Manual acceptance

Record observed evidence rather than marking an item complete because a build passed:

1. Native launch with empty local storage, then sample mode.
2. Rename a sample device, inspect local/internet filters, review evidence, and acknowledge an alert.
3. Prepare an aliased summary, edit it, verify consent resets, and switch provider. No automatic external request.
4. Import synthetic NDJSON/PCAP/Nmap XML into local storage. Exit sample mode and confirm the selected source and totals.
5. Connect to a synthetic local collector, observe its data, disconnect, and verify local storage remains separate.
6. Close/reopen the native app and verify persisted local names/review state.
7. Repeat on a real Windows device with its capture driver. A Windows CI artifact is only build evidence.
8. On an explicitly authorized topology, verify WAN and local transfers, interfaces, permissions, dropped-packet reporting, and failure recovery. No real-network validation is implied by synthetic fixtures.

No live home-network traffic, provider login/inference, or signed distribution is asserted by these tests.

## Initial alpha validation record

On 2026-09-05 UTC, the native Mac application was launched and exercised with synthetic data: empty local storage, isolated sample mode, device rename, authenticated connection to a local synthetic collector, native Nmap XML import (two discovered devices with zero traffic records), NDJSON import (840 observations), automatic selection of the imported source, local-traffic filtering, supporting alert conversations, and alert acknowledgement. Browser interaction verified that redacted summaries omit the sample endpoint identifiers and that editing the approved text revokes approval and disables sharing controls.

The hosted [initial integration run](https://github.com/CyberJorsh/home-network-security/actions/runs/33937390849) passed the real TShark four-packet synthetic import and the authenticated HTTP smoke check. Consult the latest Actions run for current source and platform results. Local checks also passed: TypeScript production build, eight frontend tests, fifteen Rust tests on Mac (including the Unix-only subprocess bound test), rustfmt, Prettier, clippy with warnings denied, and npm dependency audit. No live capture, active home-network scan, provider request, signed release, or real Windows GUI verification was performed.

## Desktop authentication and collection update

The updated macOS desktop was exercised with real, operator-authorized discovery and a timed Wi-Fi metadata capture. The capture reached the local database and the UI reported completion. No captured identifiers, addresses, payloads, credentials, or real inventories are committed. This validates host collection, not whole-network visibility, traffic completeness, or Windows driver behavior.

Official Codex 0.153.1 and Grok Build 1.0.18 passed isolated signed-out protocol checks; both issued actual device login challenges from the desktop. Grok browser consent completed and the app read the account through ACP. Authentication and capture permission setup are distinct from model inference. No model prompt was sent.

Added automated checks cover subprocess cancellation, denial of unsolicited auth-client RPC permissions, cancellation without provider output, provider URL restrictions, and API-environment exclusion. The optional installed-client check is ignored in ordinary CI because the clients are external dependencies; run `cargo test -p home-network-security installed_client_protocols -- --ignored --nocapture` explicitly where they are installed.
