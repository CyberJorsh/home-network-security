# Subscription authentication and AI integration

The required providers are **ChatGPT and Grok**, using supported subscription authentication. API keys and paid API fallback do not satisfy that requirement. Core collection and alerts must continue to work without either account.

## Real desktop sign-in

In **AI explanations**, choose ChatGPT or Grok and click **Sign in**. The app starts the installed official client with device authorization, displays its one-time code, and offers the provider's sign-in page. Finish consent in your browser. **Check session** reads the client's actual authentication state; **Sign out** clears this app's profile. Cancelling terminates the local login process; a previously issued device code can remain valid until the provider expires it.

Install the [official Codex CLI](https://developers.openai.com/codex/cli/) for ChatGPT and [Grok Build CLI](https://docs.x.ai/build/cli) for Grok. Protocol checks have been exercised with Codex **0.153.1** and Grok Build **1.0.18** on macOS. Clients are installed separately, not redistributed by this project. The desktop app resolves normal package-manager locations and npm's native Codex executable; it does not execute shell command strings. Windows installations should use an official native executable or the standard npm Codex installation. Restart after installing clients.

Each provider receives an isolated profile under the app's local data directory, in `providers/chatgpt` or `providers/grok`, and an empty working directory. Existing CLI accounts are not imported or modified. Only an allowlist of OS environment variables reaches a client. ChatGPT forces ChatGPT login with file credentials inside that profile; Grok disables API-key authentication and pins session authentication. Profile directories are private to the current user on Unix and live inside the user's application-data directory on Windows. Credentials never enter the webview. Login instructions are held in memory, not copied to repository files.

Account checks use Codex `initialize` and `account/read`, or Grok ACP `initialize` and `authenticate` with `cached_token`. They do **not** create model sessions or send prompts. These checks establish available client authentication, not remaining quota, subscription eligibility for every model, or successful inference. Expired or rejected sessions require signing in again. Requests have bounded output, deadlines, and process cleanup; unrequested RPC capabilities are rejected.

The browser preview remains synthetic and cannot perform native authentication or collection.

## Explanations still require manual submission

The app prepares at most twelve relevant conversations, aliases names and addresses by default, omits MACs and capture payloads, freezes the prepared text, and requires review of the exact text. Editing or switching the provider resets review. With aliases enabled, unknown free-form protocol labels and alert text are omitted. Copy the reviewed summary and paste it into the provider's own app when ready. Opening a provider homepage does not submit the text.

**Real subscription sign-in is implemented; embedded model inference remains gated below.** Authentication alone does not establish summary-only tool containment or prevent purchased-credit consumption. No model call, API key fallback, browser-cookie import, or automatic network-data upload is implemented.

## Official integration references

**ChatGPT:** [Codex App Server](https://learn.chatgpt.com/docs/app-server) documents a supported product-embedding protocol with managed ChatGPT browser/device authentication and account/usage inspection. [Authentication](https://learn.chatgpt.com/docs/auth) and [Windows sandbox behavior](https://learn.chatgpt.com/docs/windows/windows-sandbox) matter. Codex subscription allowances are distinct from ordinary ChatGPT conversation limits. Source is Apache-2.0; service access still has its own terms.

**Grok:** [Grok in OpenCode](https://x.ai/news/grok-opencode) establishes a supported subscription-login path. [Grok Build headless scripting](https://docs.x.ai/build/cli/headless-scripting), [CLI reference](https://docs.x.ai/build/cli/reference), and the [first-party source](https://github.com/xai-org/grok-build) offer candidate ACP/stdio integration. [Enterprise configuration](https://docs.x.ai/build/enterprise) documents API-key authentication precedence and disabling API-key auth. Do not inherit ambient API credentials into a subscription-only client.

Grok [subscription policies](https://docs.x.ai/grok/faq) describe shared allowances and possible use of purchased credits. A subscription login by itself does not prove a request cannot incur credit use. [Sandbox documentation](https://docs.x.ai/build/features/sandbox) also shows differences across platforms; read-only does not mean the client can read only the approved summary. In-process network tools may be outside child-process sandbox restrictions.

## Required acceptance checks

1. Complete operator browser consent, account display, logout and re-authentication on both macOS and Windows; verify subscription eligibility. Signed-out protocol tests and device-code issuance alone do not complete this gate.
2. Only the exact reviewed text enters a request. A new request needs fresh review; background polling cannot trigger inference.
3. Disable shell, filesystem, web search, subagents, memory, environment instructions, and unrelated workspace access through supported controls. Use an isolated directory and scrub ambient API credentials. Demonstrate containment with adversarial tests; a working login is insufficient.
4. Inspect quota before requests where supported, fail closed on exhausted/unknown billing mode, and prevent automatic purchased-credit or API fallback. No UI claim of subscription-only operation without evidence.
5. Cancellation, deadlines, process cleanup, bounded protocol messages, and prompt/response log retention controls.
6. Present provider output as untrusted explanation tied to local evidence. Never execute its commands or turn it into network blocking.
7. Resolve distribution/license notices and expose a clear supported-client version range.

A developer protocol spike must use synthetic reviewed text. Live account sign-in and inference require the operator's own authentication and an explicit reviewed submission. Until these gates are met, the app keeps embedded inference disabled.
