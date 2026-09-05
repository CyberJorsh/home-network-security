# Contributing

Small, focused pull requests are welcome. Explain the user-visible behavior, how you tested it, and the platform or coverage limits you could not verify.

Use current stable Rust and Node.js 22.12+. Run the commands under **Contribute** in the README. The desktop application requires native build prerequisites. The core and collector can be tested on Linux without compiling Tauri with `cargo test -p hns-core -p hns-collector`.

Keep source files formatted with Prettier and rustfmt. Add tests for meaningful accounting, parsing, persistence, privacy, and failure behavior. Do not weaken types to satisfy a build.

## Boundaries

- Never commit captures, databases, credentials, real network inventories, provider sessions, or personal project notes. Use synthetic fixtures with documentation addresses and locally administered MACs.
- Capture and scanning must remain explicit actions. Do not introduce blocking, router writes, automatic cloud sharing, or paid inference fallback.
- Every alert needs inspectable evidence and honest uncertainty. A known port, hostname, or vendor hint is not proof of identity or compromise.
- Preserve Windows/macOS support and hardware coverage gaps. Avoid depending on a specific router or an always-visible switch port.
- Provider access must use supported ChatGPT/Grok subscription authentication. See `docs/providers.md` before proposing an integration.
- Do not bundle Nmap, Npcap, or other external executables without separately resolving their redistribution requirements.

Use issues for reproducible bugs and scoped proposals. Report security problems through GitHub's private vulnerability reporting feature, not a public issue containing private data.

There is no contributor license agreement. By submitting a contribution, you agree it is offered under this repository's Apache-2.0 license.
