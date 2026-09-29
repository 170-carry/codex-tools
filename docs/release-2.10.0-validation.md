# v2.10.0 maintenance review

All nine open issues and both open pull requests were reviewed against `main` at `8a7b7ad` on 2026-09-29.

| Item | Resolution |
| --- | --- |
| #209 / #211 | Accept the macOS bundle discovery, file credential store and verified shutdown/relaunch fix. Also bypass the old no-op path when the installed profile still uses another credential store. Extract launch, process inspection and switch persistence into separate modules. |
| #208 / #207 | Accept GPT-6 Sol/Luna model catalog, aliases, permissions and Responses Lite support. Preserve explicit key allowlists, restrict generic account failover to server errors, and invalidate older cost caches. |
| #202 | Generate an HTTP-only `codex_tools_relay` provider. Keep the legacy OpenAI base URL for older sessions. Switching back to ChatGPT removes the managed Relay provider. |
| #201 | Add saved Relay editing with optional key replacement. Keep account identity, import time and proxy selection; a blank key retains the secret. Apply desktop routing on the next explicit account switch. |
| #203 | Use immutable import order for account rows while preserving quota ranking for smart switch. |
| #204 | Select quota-icon percentages from the chosen 5h or weekly window. Combined display continues to show the limiting remaining quota. |
| #205 | Translate `response.done`, incomplete and failed events, preserve upstream errors, and report a truncated stream instead of silently sending `[DONE]`. The reporter's third-party client still needs a live retest. |
| #199 | Require complete structured terminal events, distinguish unused windows from active windows, retain cooldowns and report request completion separately from quota-confirmed activation. Actual quota activation is controlled by the upstream service. |
| #206 | Remains open. Could not reproduce settings disappearance when toggling OpenCode sync and desktop restart in Chinese, English, Japanese, Korean or Russian. Need the reporter's app version, Windows/WebView details and reproduction/error log. |

## Local validation

- Rust library: 308 tests passed, including macOS discovery/process matching, shared profile configuration, stream translation, warm-up eligibility and Relay editing.
- Frontend: 26 tests passed with a TypeScript-capable Node runtime; production build passed.
- ESLint: zero errors, four existing unused-disable warnings.
- Standalone proxyd: locked dependency compilation passed from the actual `gen/remote-build` installer resources. Installer bundling and deployment copying now use one complete module manifest.
- Browser smoke: five locales, OpenCode enable/restart/disable, and Relay URL editing with blank-key retention; zero page errors. Native Tauri commands were mocked for this UI check.
- Model prices checked against [GPT-6 Sol](https://developers.openai.com/api/docs/models/gpt-6-sol) and [GPT-6 Luna](https://developers.openai.com/api/docs/models/gpt-6-luna). Credential-store and provider flags checked against the [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference).

No production database was queried or modified. The changes add no SQL or schema migrations. Existing SQLite tests use bounded temporary fixtures; their migrations and aggregation queries do not scan user databases. No real inference request or account switch was performed in the maintainer's active desktop during local verification. Native macOS/Windows regression gates must pass before the release workflow publishes installers and npm packages.
