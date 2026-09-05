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

## Reviewed streaming and guided setup update

On macOS, real subscription calls using Codex 0.153.1 and Grok Build 1.0.18 completed with synthetic text: 82 ChatGPT chunks and 87 Grok chunks in the backend smoke test. Both also completed reviewed sends in the native UI, with restored sessions, compact account controls, actual model/effort selection, and review reset after send. Synthetic adversarial probes for each provider observed no canary disclosure, marker-file modification, local HTTP connection, or tool notification. No real network summary was sent in these developer tests.

Frontend interaction tests cover automatic session checks, hidden sign-in controls, editing an empty summary, exact reviewed sends, effort/provider review reset, streamed rendering, and install prompts from both collection buttons when tools are absent. Rust tests cover quota refusal, missing billing data, paid-credit refusal, protocol cancellation, permission rejection, and open-service/OS evidence parsing. Installation launch is implemented for both desktop targets; physical Windows prompts and fresh-machine macOS Homebrew bootstrap remain unverified. Existing capture tools were preserved on the development Mac.

Opt-in tests use the operator's isolated app provider directory and consume subscription allowance. Ordinary CI does not run them:

```sh
HNS_TEST_PROVIDER_ROOT=/path/to/app/providers cargo test -p home-network-security real_synthetic_streams -- --ignored --nocapture
HNS_TEST_PROVIDER_ROOT=/path/to/app/providers cargo test -p home-network-security real_containment_canaries -- --ignored --nocapture
```

Keep live account metadata, inventories, captures, and private test artifacts out of public reports. A passing synthetic containment probe is not a complete security audit or proof of Windows runtime behavior.

The updated macOS UI also completed an explicit service scan of the local computer and displayed the observed service/product/version in its device details and review summary. OS identity enrichment is tested to apply only to this computer's own interface addresses; neighboring devices retain unknown values.
