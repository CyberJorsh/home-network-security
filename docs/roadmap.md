# Roadmap and acceptance gates

The source is an early functional alpha. Publishing it does not complete the full product brief.

| Area                     | Current implementation                                         | Remaining acceptance gate                                                                                     |
| ------------------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| Windows + Mac desktop    | Shared Tauri/React source and native CI build jobs             | Real Windows GUI/driver exercise; signed/notarized installers and full third-party distribution notices       |
| Device discovery         | Explicit Nmap CLI and native XML import                        | Guided discovery UI, interfaces/permissions diagnostics, IPv6 discovery strategy                              |
| Internet + local traffic | TShark adapter and accounting for both directions              | Physical test topology proving both WAN and between-device coverage, packet-drop telemetry                    |
| Hardware flexibility     | Local/remote collector, SSH tunnel, explicit coverage gaps     | Verified router/AP/export adapters and deployment recipes on actual supported hardware                        |
| Device inventory         | IP/MAC evidence, labels, persistence                           | DHCP, multi-address, and cross-sensor identity reconciliation with ambiguity preserved                        |
| Traffic history          | Bounded event storage and recorded-window views                | Rollups, configurable time range, retention controls, sustained-rate measurements                             |
| Alerts                   | Evidence-backed observed-device and large-upload observations  | User-configurable rules, baseline/change detection and false-positive evaluation                              |
| Optional subscription AI | Subscription login, reviewed streaming, model/effort selection | Physical Windows account/inference validation; independent containment review; broader client-version support |
| Local privacy            | No telemetry/cloud backend; native storage boundary            | Local-data export/reset UI, installation review, independent security review                                  |

Use repository issues to scope these gates. A gate is closed by direct evidence for the target environment, not by changing this table or passing a compiler.

Tracked work: [subscription AI](https://github.com/CyberJorsh/home-network-security/issues/1), [real coverage and Windows device validation](https://github.com/CyberJorsh/home-network-security/issues/2), [signed distribution](https://github.com/CyberJorsh/home-network-security/issues/3), and [guided collection and history](https://github.com/CyberJorsh/home-network-security/issues/4).
