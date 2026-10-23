# v3.1.3 review and validation

Reviewed on 2026-10-23 against v3.1.2 and PR #235 at
`53792010c7e50987ca95f13be4e47a5535ad0a38`.

## PR review

Adopt the bounded quota refresh/retry implementation, shared batch HTTP client,
eight-worker cap, partial-failure notice, number precision and verified Windows
Store activation changes from #235. Split HTTP transport, response mapping and
tests into independent modules and include them in packaged proxyd resources.

Do not adopt the automatic switch/continue workers, app control socket, UI
Automation scripts or bundled DLL. Automatically deleting all thread writer
locks can conflict with other live clients; sending a follow-up to an existing
task through UI Automation is not sufficiently validated across platforms.
The draft's custom-provider filter also changes the meaning of global local
Token totals, so it is excluded. The original contributor is credited in the
integration commit. The draft is superseded by the scoped integration PR.

## Issue decisions

| Issue | Decision |
| --- | --- |
| #232 | Fix lost Relay definitions when `model_providers` is an inline TOML table; validate complete provider settings and keep definitions referenced by saved profiles. A standalone reproduction confirms the old serialization drops the provider for `model_providers = {}`. |
| #227 | Add independent main-window geometry persistence. Restore normal size, position and maximized state; clamp offscreen/oversized geometry to connected displays. Update memory on moves/resizes; write on focus loss, close and exit. |
| #234 | Clarify session-creator attribution visibly in the account Token card and tooltip. Continuing a session after changing accounts does not rewrite its creator; local logs cannot reliably reconstruct actual account billing. Keep total Token semantics and do not fabricate per-account charges. |
| #206 | Already fixed by #220, released in v3.0.3, with explicit confirmation from the reporter on 2026-10-14. Close as completed without duplicating the fix. |
| #205 | The reported missing stream completion was addressed in v2.10.0. Current regressions cover completion and error handling; no new raw events or reproducible failure were supplied after the request for retesting. Close the old report with the existing fix and a request for a new reproducible report if it persists. |
| #229 | Do not add a second mobile pairing/session-control or iCloud widget system to this desktop account tool. Retain the documented proxy feasibility experiment; real-phone acceptance is not claimed. Close as not planned with reasons and official Remote documentation. |

## Validation

- Rust library: 357 tests passed, including quota retries/budgets, inline/dotted
  provider definitions and window geometry. TZ=UTC matches existing fixture dates.
- Frontend: 36 unit tests, 33 page-render and 25 layout-render checks passed.
- Headless browser: 29 interaction checks passed with no runtime errors.
- ESLint, production build, Rust formatting and diff whitespace checks passed.
- Packaged standalone proxyd passed the locked Cargo check with all new modules.
- Existing dead-code and bundle-size warnings remain.
- New modules stay below 400 lines; no opaque DLL is added.
- No new SQL. Existing database tests use disposable local fixtures; timestamp
  indexes/ranges, primary-key lookups and bounded transactional writes were
  inspected before testing. No installed account database or credentials were
  used for these tests.
- No real reset card was consumed, no live session was resumed and no production
  account was switched. Native Windows and real-phone behavior are not claimed
  from local macOS/browser tests. Release CI also compiles/tests Windows.

## Publication

Use release.yml with release_tag=v3.1.3 and publish_npm=false. Verify all three
desktop builds, installer hashes, updater signatures and public latest.json.

## References

- [Authentication](https://learn.chatgpt.com/docs/auth)
- [Remote connections](https://learn.chatgpt.com/docs/remote-connections)
- [Sign in with ChatGPT through a gateway](https://learn.chatgpt.com/docs/enterprise/sign-in-with-chatgpt-through-a-gateway)
