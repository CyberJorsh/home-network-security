# Third-party software

Our code is Apache-2.0. Dependency licenses apply independently; the lockfiles identify the exact source versions used. Distribution maintainers must preserve applicable notices when shipping binaries.

Principal libraries include Tauri (MIT/Apache-2.0), React and Vite (MIT), lucide (ISC), rusqlite (MIT), and bundled SQLite (public domain). This is an orientation, not an exhaustive binary notice manifest. Signed releases remain gated on a complete distribution inventory and license review.

External tools are not bundled:

| Tool                     | Purpose                                                  | Source and terms                                                                                                  |
| ------------------------ | -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Wireshark/TShark/dumpcap | Decode or capture packet metadata                        | [Wireshark](https://www.wireshark.org/docs/wsug_html_chunked/PrefaceLicense.html), GPL-2.0-or-later               |
| Nmap                     | Explicit host discovery / XML import                     | [Nmap licensing](https://nmap.org/book/man-legal.html), NPSL; redistribution/integration requires care            |
| Npcap                    | Windows capture driver when applicable                   | [Npcap licensing](https://npcap.com/oem/); ordinary installer permission does not grant OEM redistribution rights |
| Codex / Grok Build       | Candidates for future supported subscription integration | See [provider notes](docs/providers.md); not embedded or redistributed in this alpha                              |

Separate executable installation does not by itself resolve every integration or redistribution obligation. Do not add these binaries to the application bundle without reviewing their actual terms. Provider accounts, subscription access, trademarks, and service terms are independent of this project's source license.

Official Codex CLI and Grok Build CLI are optional, separately installed authentication clients. This application invokes their supported login and local authentication protocols; their binaries are not included in the application bundle. See [Codex source and license](https://github.com/openai/codex) and [Grok Build source and license](https://github.com/xai-org/grok-build), as well as each provider's service terms. Subscription authentication does not grant redistribution rights or unlimited inference.
