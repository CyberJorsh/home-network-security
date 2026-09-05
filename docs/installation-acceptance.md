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
6. Save a synthetic explanation, reopen, inspect its exact summary, and delete that saved item. Export disposable observations and open the delete-all confirmation and cancel without typing. Confirm deletion by typing DELETE LOCAL DATA only in a disposable test profile.
7. Run controlled WAN and between-device transfers from a known topology. Compare capture totals and packet drops against the generating endpoints; do not mark whole-network coverage verified without this evidence.

Current gate: no Developer ID Application identity was available on the development Mac during the September 2026 reliability update. Physical Windows GUI/driver and fresh-machine Homebrew bootstrap acceptance remain pending. Unit tests exercise installer routing and resumption; CI builds exercise both desktop targets.

## Rechecked during live home validation

The September 5, 2026 live-host validation rechecked the available signing identities and ran the release verifier against the rebuilt native Mac app. Only Apple Development identities were available; the verifier correctly rejected the ad-hoc application as unsuitable for direct signed distribution. No certificate was created or changed, and no installer was published.

A complete binary notice inventory also remains an explicit release gate. Declared package licenses in a lockfile are not the complete copyright notices and license texts owed by an actual distributed artifact. Audit the resolved dependencies for each release target, collect their required notices (including native/bundled transitive components), include the resulting notice material with the artifact, and review the exact bundle contents. Nmap, TShark, Npcap, and provider clients remain separately installed tools; do not add their binaries to the application as a shortcut.

Issue #3 must remain open pending maintainer-controlled distribution signing, notarization, the complete artifact notice review, and physical Windows/fresh-machine installation and uninstall acceptance. Successful local launch and the negative unsigned-artifact check do not close it.
