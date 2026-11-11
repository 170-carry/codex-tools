# v3.2.1 validation

Desktop account switching continues through the ordinary account-switch path when local session state cannot be read, parsed or written. The UI reports skipped session sync. Credential failures and unfinished transactions still stop the switch.

The approach follows [Magpie's separation of project sync from account switching](https://github.com/yetone/magpie/blob/main/internal/provider/codex_projects.go). Existing rollback, credential-refresh and pending-transaction behavior is retained.

- Rust library: 487 passed. Compiled with `cargo test --locked --lib --no-run`, then ran the resulting binary through `scripts/test-isolated.sh` with synthetic directories and access to real account data blocked.
- Node 24 unit tests: 62 passed, including 19 account/session flow tests.
- ESLint, TypeScript/Vite build and 82 page/layout checks: passed.
- Packaged standalone proxy: `cargo check --locked` passed.
- Failure cases cover malformed JSON, UTF-8 BOM, unreadable checkpoints, session-write rollback, concurrent session changes, invalid credentials and pending recovery. A successful sync does not invoke a second switch; an ordinary switch failure remains an error.
- Logs include operation, file path, JSON category/line/column and filesystem failure details. JSON schema errors omit input values to avoid logging credentials.
- Successful sync adds no file reads or polling. Error classification reuses existing transaction checks. Database tests use bounded temporary fixtures; no real account database was queried or modified.

Native Windows behavior and real-account switching were not exercised locally. The release workflow validates macOS and Windows before building signed Apple Silicon, Intel and Windows desktop artifacts. npm publication is disabled for this desktop release. Existing unused-code and Vite chunk-size warnings remain.
