# Subscription AI integration: implementation gate

The required providers are **ChatGPT and Grok**, using supported subscription authentication. API keys and paid API fallback do not satisfy that requirement. Core collection and alerts must continue to work without either account.

## Current alpha

Two provider choices are available. The app prepares at most twelve relevant conversations, aliases names and addresses by default, omits MACs and capture payloads, freezes the prepared text, and requires review of the exact text. Editing or switching the provider resets that review. With aliases enabled, unknown free-form protocol labels and alert text are omitted to prevent identifiers hidden in metadata from bypassing redaction. Copying uses the local clipboard. Opening a provider uses only its fixed homepage URL; it does not submit the text.

This is a useful manual workflow, **not completed embedded subscription AI**. There is no fake login button, OAuth credential scraping, imported browser cookie, automatic request, or API-billing substitute.

## Supported paths under investigation

**ChatGPT:** [Codex App Server](https://learn.chatgpt.com/docs/app-server) documents a supported product-embedding protocol with managed ChatGPT browser/device authentication and account/usage inspection. [Authentication](https://learn.chatgpt.com/docs/auth) and [Windows sandbox behavior](https://learn.chatgpt.com/docs/windows/windows-sandbox) matter. Codex subscription allowances are distinct from ordinary ChatGPT conversation limits. Source is Apache-2.0; service access still has its own terms.

**Grok:** [Grok in OpenCode](https://x.ai/news/grok-opencode) establishes a supported subscription-login path. [Grok Build headless scripting](https://docs.x.ai/build/cli/headless-scripting), [CLI reference](https://docs.x.ai/build/cli/reference), and the [first-party source](https://github.com/xai-org/grok-build) offer candidate ACP/stdio integration. [Enterprise configuration](https://docs.x.ai/build/enterprise) documents API-key authentication precedence and disabling API-key auth. Do not inherit ambient API credentials into a subscription-only client.

Grok [subscription policies](https://docs.x.ai/grok/faq) describe shared allowances and possible use of purchased credits. A subscription login by itself does not prove a request cannot incur credit use. [Sandbox documentation](https://docs.x.ai/build/features/sandbox) also shows differences across platforms; read-only does not mean the client can read only the approved summary. In-process network tools may be outside child-process sandbox restrictions.

## Required acceptance checks

1. Official sign-in, sign-out, account display, expiry/re-authentication, and subscription eligibility on both macOS and Windows.
2. Only the exact reviewed text enters a request. A new request needs fresh review; background polling cannot trigger inference.
3. Disable shell, filesystem, web search, subagents, memory, environment instructions, and unrelated workspace access through supported controls. Use an isolated directory and scrub ambient API credentials. Demonstrate containment with adversarial tests; a working login is insufficient.
4. Inspect quota before requests where supported, fail closed on exhausted/unknown billing mode, and prevent automatic purchased-credit or API fallback. No UI claim of subscription-only operation without evidence.
5. Cancellation, deadlines, process cleanup, bounded protocol messages, and prompt/response log retention controls.
6. Present provider output as untrusted explanation tied to local evidence. Never execute its commands or turn it into network blocking.
7. Resolve distribution/license notices and expose a clear supported-client version range.

A developer protocol spike must use synthetic reviewed text. Live account sign-in and inference require the operator's own authentication and an explicit reviewed submission. Until these gates are met, the app keeps embedded inference disabled.
