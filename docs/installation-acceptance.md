# Installation and distribution acceptance

Development CI artifacts remain unsigned. A successful build does not validate a fresh machine or Npcap permissions.

## Signing

Use the documented [Tauri macOS signing and notarization process](https://v2.tauri.app/distribute/sign/macos/) or [Windows code signing process](https://v2.tauri.app/distribute/sign/windows/). macOS direct distribution requires a Developer ID Application identity and notarization credentials; Windows requires a suitable code-signing certificate or signing service. Keep credentials outside the repository.

Build with the configured signing identity, then verify the exact artifact:

```sh
python3 scripts/verify_distribution.py 'target/release/bundle/macos/Home Network Security.app'
```

On Windows, pass the signed NSIS `.exe` to the same script. It checks Authenticode status and a timestamp. The Mac path checks Developer ID signing, strict code validation, stapling, and Gatekeeper acceptance. The script fails for development/ad-hoc artifacts and never publishes anything.

## Fresh-machine exercise

Record OS, architecture, app commit, tool versions, each observed result, and any failures. Use disposable synthetic imports and a network/interface explicitly selected by the operator.

1. Install and launch on a clean Mac and a physical Windows PC. Confirm the signature/publisher and normal OS launch behavior.
2. With Nmap absent, click Discover, inspect the installer prompt, cancel once, retry, finish installation, and Continue. Confirm exactly one discovery resumes for the selected range.
3. With TShark absent, click Start capture, finish setup, Continue, choose an interface, and start. On Windows exercise Npcap both with and without account permission. Permission/driver errors must show their specific remedy; a timeout must not reinstall tools.
4. Dismiss a failed installer setup and retry. Stop a running capture and verify collected metadata, completion, and any reported drop count. Unknown drops must stay unknown.
5. Sign into each supported provider. Send only a reviewed synthetic prompt. Navigate away while it streams, return, and verify the answer completes and models become available. Reopen the app and verify provider/model/effort preferences.
6. Save a synthetic explanation, reopen, inspect its exact summary, and delete that saved item. Export disposable observations and exercise Cancel on the delete-all confirmation. Confirm deletion only in a disposable test profile.
7. Run controlled WAN and between-device transfers from a known topology. Compare capture totals and packet drops against the generating endpoints; do not mark whole-network coverage verified without this evidence.

Current gate: no Developer ID Application identity was available on the development Mac during the September 2026 reliability update. Physical Windows GUI/driver and fresh-machine Homebrew bootstrap acceptance remain pending. Unit tests exercise installer routing and resumption; CI builds exercise both desktop targets.
