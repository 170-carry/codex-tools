# v3.1.0 validation

Prepared on 2026-10-13 from PR #225 (`15c156c`) and the additional #226 fix.
The release uses an isolated Git worktree; uncommitted classic-layout changes in
the primary checkout are outside this release.

## Changes

- #221: macOS proxy-pool average quota and known/total account counts.
- #222: local-time automatic warm-up windows, including overnight windows.
- #223: stable account-group ordering and independent priority routing.
- #224: explicit reset-credit selection, confirmation and persisted idempotency.
- #226: manual warm-up bypasses recent-success and recent-failure cooldowns;
  automatic warm-up retains them. Both paths retain active-window and exhausted
  weekly-quota checks, and the request path retains its usage/auth checks.
- Priority routing is isolated in `proxy_service/priority.rs` and included in the
  shared standalone-proxy build manifest. Reset-credit UI operations live in
  `useResetCredits.ts`, including synchronous duplicate-submission protection.

## Local gates

- Desktop Rust library: **330 passed**, including manual/automatic cooldown
  boundaries and unchanged active/exhausted-window behavior.
- Packaged standalone proxyd source: **192 passed** with `--locked`; this tests
  the generated installer resources rather than only the original source tree.
- Frontend unit tests: **41 passed**.
- Static rendering: **43 page checks + 30 layout checks passed**.
- TypeScript/Vite production build, ESLint, Rust formatting and diff whitespace
  checks passed. Vite retains its existing main-chunk-size warning.
- Extracted bilingual notes match the v3.1.0 changelog entry. App, Tauri, Cargo
  and npm package metadata all use 3.1.0.

## Browser checks

Microsoft Edge on macOS, local preview with synthetic accounts and mocked Tauri
commands:

- Classic and compact layouts: move an account group, open credit confirmation,
  and redeem a simulated card; the available count falls from three to two.
- Cancelling the classic confirmation leaves no confirmation action and does
  not consume a card.
- Enable automatic warm-up and its schedule; set 07:30–23:00 and read it back.
- Enable the macOS proxy-pool display setting.
- Select the priority proxy strategy and reopen the selector to verify it is
  selected.
- Windows preview mode with OpenCode installed: enabling sync shows the restart
  option and keeps Settings visible; classic keyboard toggling also keeps the
  page visible. No application console errors were observed. Browser-extension
  warnings are unrelated to the preview application.

These checks do not constitute native Windows/WebView acceptance. The existing
production Codex Tools app was running on this Mac, so it was not interrupted
for an isolated native menu-bar acceptance run. No real reset credit was used;
redemption HTTP tests use only a local mock upstream.

## SQL impact review

No SQL or schema was added or changed. Before running the existing Rust tests,
usage-store and provider-sync SQL were reviewed. Tests create small databases in
unique temporary directories: event queries use timestamp indexes and bounded
ranges/limits, metadata lookups use the primary key, and writes use transactions.
The provider rewrite scans only the two-row test fixture; full scans and manual
clear tests do not touch an installed application's data. No user database was
queried or modified by the release verification.

## Publication and outstanding reports

The GitHub workflow validates macOS and Windows before building Apple Silicon,
Intel and Windows desktop packages. This release is dispatched with
`release_tag=v3.1.0` and `publish_npm=false`, as requested. The action creates the
release tag at the validated workflow commit; desktop publication must be
verified through the workflow result, release assets and updater manifest.

#205 (Luna client failure) and #206 (native Windows settings visibility) remain
open: no new original error events or current-version native reproduction were
provided. Existing protocol regressions and browser previews cannot establish
that these reports are fixed. The separate P0/P1 diagnostic/setup plan in PR #225
remains a plan rather than a delivered feature.
