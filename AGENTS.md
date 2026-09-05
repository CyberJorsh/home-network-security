# Repository working agreements

- Preserve both macOS and Windows as desktop targets. The collector may also run on Linux.
- Never publish PROJECT_BRIEF.md, captures, databases, tokens, real network inventories, or provider sessions. Use synthetic fixtures.
- Collection and discovery are explicit user actions. No blocking, router writes, automatic sharing, or API billing fallback.
- Keep sensor coverage unverified until supported by direct topology evidence. Tests/builds are not real-network proof.
- ChatGPT and Grok must use supported subscription authentication. Embedded inference remains gated by docs/providers.md.
- Run appropriate tests, TypeScript checks, rustfmt/Prettier, and clippy for changes. Keep public status honest about remaining acceptance gates.
