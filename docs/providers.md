# Subscription explanations

The desktop app uses **ChatGPT through Codex 0.153.1** and **Grok through Grok Build 1.0.18**. Install the [official Codex CLI](https://developers.openai.com/codex/cli/) or [official Grok Build CLI](https://docs.x.ai/build/cli), then restart the app. These exact client versions are currently required for embedded inference because their protocol and tool restrictions were tested. New versions need validation before widening support. Clients are installed separately, not redistributed.

## Sign in and send

1. In **AI explanations**, choose ChatGPT or Grok. Complete the official device-code browser consent when signing in. Existing app sessions are checked automatically; connected accounts collapse into **Account settings** with check/sign-out controls.
2. Choose a model and reasoning effort from the account's live catalog. Higher effort may take longer and use more allowance. ChatGPT uses Codex subscription allowances, which differ from ordinary ChatGPT conversation limits.
3. Select a device and **Prepare summary**. It includes names, addresses, MAC, reported hostname/vendor, available model/OS hints, discovered services, observation times, totals, and at most twelve relevant conversations. Unobserved details are labeled. Optional privacy mode aliases addresses and omits identifying free-form fields. No capture payloads or unrelated devices are included.
4. Edit the text, check **I reviewed this exact summary**, and **Send to ChatGPT/Grok**. Only this frozen text plus fixed explanation instructions is submitted. Editing, preparing, changing provider/model/effort, or sending resets review. Empty summaries cannot be sent.
5. Read the response as it streams inside the app. **Stop** cancels the local request. A stopped/failed response is labeled incomplete; retries require a new reviewed click. There is no automatic retry, provider switch, API-key fallback, or automatic sharing.

Core collection and alerts work without either account. The browser preview is synthetic and cannot authenticate, collect, or send.

## Authentication and billing boundaries

Provider credentials remain in isolated app-owned profiles under the OS application-data directory. The webview never receives access/refresh tokens. Ambient API credentials, client overrides, and agent context are excluded from child environments. ChatGPT login is forced to ChatGPT; Grok API-key authentication is disabled and authentication uses its cached subscription token.

Each send checks authentication, model/effort validity, and live usage before creating a prompt. ChatGPT requires available primary/secondary allowance and no purchased or unlimited-credit balance. Grok requires unified subscription billing, remaining included usage, zero prepaid balance, and zero on-demand cap. Unknown, exhausted, or paid-fallback configurations stop before submission with an explanation. The app never purchases/redeems credits or changes provider billing settings. Accounts with paid credits enabled must resolve that in the provider's own settings to use this strict subscription-only path.

Account and catalog reads send no model prompt. Usage inspection is a preflight snapshot, not a reservation of allowance against other concurrent account activity. Provider limits and policies still apply.

## Model containment and retention

Requests run in a private disposable profile and empty directory, with only the app's provider credential file copied in. Refreshed credentials are copied back while an account-operation lease prevents overlapping login/logout. Other app histories, real inventories, home files, and workspace instructions are not supplied.

Codex uses its supported custom model catalog with patch and experimental tools removed. Shell, filesystem helpers, web/search, apps/plugins, subagents, memory, image/computer/browser tools, and workspace/environment instruction discovery are disabled. Sessions are ephemeral, read-only, approval-never, and use the default service tier.

Grok uses an explicit curated agent profile with default-tool injection, skills, AGENTS files, MCP inheritance, and memory disabled. Grok rejects an empty curated toolset, so the sole registered tool is its session-local in-memory TODO operation. It has no filesystem, command, network, memory, or subagent access. Before prompting, the app verifies the selected profile name and a tool-definition count of one. Any tool notification or client capability request stops the explanation. The prompt uses ACP's verbatim flag.

Protocol output is capped at 8 MiB, individual messages at 256 KiB, submitted text at 64 KiB, and rendered response at 256 KiB. RPC setup steps have 30-second deadlines; generation has a five-minute deadline. Cancellation kills/reaps the official process. Provider-requested client capabilities are rejected. Responses are rendered as inert text; commands and links are never executed.

Temporary client profiles are removed after completion, cancellation, or handled failure. An OS crash or forced kill can leave temporary files under `providers/requests`; they remain in the private application-data directory and can be removed with the app stopped. The latest response is held in app memory until exit or another request. Provider-side data retention follows the provider's policies. Local deletion is not a forensic erasure guarantee.

## Verification and remaining gates

On macOS, both real signed-in subscriptions produced streamed responses to synthetic text through the backend and native UI. Synthetic adversarial probes attempted file reads/writes, a local HTTP fetch, and subagent use; no canary content, marker write, HTTP connection, or tool notification was observed. Automated tests cover quota refusal, exact reviewed payloads, review reset, restored-session controls, and missing-tool setup. These checks are bounded evidence, not an independent security audit.

Physical Windows login, inference, installation prompts, and capture-driver behavior remain to be exercised. CI builds do not close those gates. See [verification](verification.md) for commands and [roadmap](roadmap.md) for broader alpha limits.

## Official integration references

- [Codex App Server](https://learn.chatgpt.com/docs/app-server) and [authentication](https://learn.chatgpt.com/docs/auth).
- [Grok Build headless scripting](https://docs.x.ai/build/cli/headless-scripting), [CLI reference](https://docs.x.ai/build/cli/reference), and [first-party source](https://github.com/xai-org/grok-build). ACP extension method names have an underscore prefix on the wire, for example `_x.ai/billing`.
- [Grok subscription policies](https://docs.x.ai/grok/faq) and [enterprise authentication configuration](https://docs.x.ai/build/enterprise).

## Local preferences and optional history

Provider/model/effort choices are stored locally without credentials. Model availability is checked again when returning to the page; an in-progress request holds the account lease and model loading waits for it. Status requests are serialized, with slower idle polling.

Completed explanations are saved only when the user clicks Save completed explanation. The local database keeps up to 20 exact request/response pairs; duplicate saves do not add duplicates. Users can inspect and delete saved items or export them with local data. Responses render a small Markdown subset as React text, without raw HTML, remote images, or executable links. Client version containment gates remain in force.
