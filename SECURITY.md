# Security policy

This is experimental alpha software. There is no promise of complete monitoring, attack prevention, a security audit, or a response SLA.

## Report a vulnerability

Use this repository's **Security → Report a vulnerability** feature when enabled. Do not attach real captures, credentials, tokens, network inventories, or personal information to public issues. If private reporting is unavailable, open a public issue asking for a private reporting channel without vulnerability details.

## Trust boundaries

Capture and discovery run only through explicit CLI commands. Nmap discovery is limited to one private IPv4 /24 or smaller. External tool arguments use subprocess argument arrays, not a shell. PCAP and XML are untrusted input; size/output limits, deadlines, validation, and transactional ingestion reduce exposure but do not replace patched external tools.

Only normalized IP metadata is stored. It still reveals private relationships and activity. SQLite is not encrypted by the application. Database files are created with mode 0600 on Unix, with SQLite sidecar files inheriting database permissions. On Windows, files rely on the current user's application-data directory ACL. Do not use a shared database directory or run the desktop app as administrator/root. Grant the external capture component the minimum OS-supported permissions instead.

The collector API binds only to 127.0.0.1, requires a bearer token, sends no CORS headers, and uses no cookies. Tokens are generated locally and never printed. Unix token files are created mode 0600; protect existing token files and Windows directory ACLs yourself. Use SSH forwarding for another host; do not expose this HTTP service with an unauthenticated proxy. The native app accepts only a loopback port and token, keeps the token in memory, disables HTTP proxies/redirects, and bounds responses.

The API is intended for a trusted local user, not a hostile multi-user server. Other software running as that user can read files, credentials, or the clipboard. The app is not an isolation boundary against a compromised host.

The webview has a restrictive content security policy and no generic filesystem, shell, or remote-network plugin. Native commands expose only app-specific actions. Browser preview data is synthetic. The app has no telemetry, automatic cloud synchronization, or embedded inference in this alpha. Opening an AI provider sends no summary; copying places only the reviewed text on the OS clipboard. The user controls pasting it into a provider, where that provider's policies apply.

Retention is event-count based, not a secure erasure guarantee. Deleted SQLite rows can remain in free pages/WAL/backups. Removing the local data directory with the app and collector stopped is a user-managed reset, not forensic erasure.
