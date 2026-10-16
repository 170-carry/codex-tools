# v3.1.1 validation

Prepared on 2026-10-16 from the full classic account UI/token-statistics working
set, rebased onto the v3.1.0 main branch. Includes the local preview launch config.

## Changes

- Refresh classic account summaries, rows, plan styles and responsive details.
- Attribute local 24-hour/7-day/30-day Token totals to session creator accounts;
  show estimated output TPS, last activity, response counts and Credits balances.
- Keep account aggregation and timing in `token_usage/account_usage.rs`; split
  frontend detail headers, statistics, token cards, switch history and CSS.
- Preserve v3.1.0 account-order and reset-credit actions; give reset-card actions
  a dedicated grid column to prevent vertically wrapped button labels.
- Bump app/package metadata, Tauri and Cargo to 3.1.1. Leave npm distribution
  package versions unchanged and dispatch desktop publication with npm disabled.

## Local checks

- Rust library: 332 passed. The first run had a transient missing temporary file
  in the existing OpenCode provider-auth test (331 passed); its isolated retry
  and a subsequent complete suite both passed. No OpenCode code was changed.
- Frontend unit tests: 41 passed using Node 24.19.0. The host's default Node 20
  cannot run the TypeScript tests directly; release CI uses Node 22.
- ESLint, TypeScript/Vite production build and Rust formatting passed. Existing
  dead-code and Vite chunk-size warnings remain.
- Static rendering: 43 page checks and 32 layout checks passed, including
  per-account statistics and compatibility with older snapshots without them.
- Headless Chrome with synthetic accounts/mocked Tauri: 61 interaction checks
  passed, plus 9 classic account checks at 1586x992 and 640x500. No page runtime
  errors or horizontal page overflow; reset-card button stays horizontal.
- Packaged standalone proxyd: locked Cargo check passed.
- Bilingual release-note extraction selects the v3.1.1 entry.

## Data/performance and acceptance boundaries

No new SQL or schema changes. The added statistics reuse the incremental local
JSONL scan/cache and aggregate confirmed deltas; creator-less sessions stay in
global totals and do not receive a fabricated account attribution. Speed is a
local-log timing estimate, not a server generation-speed measurement.

Before executing Rust tests, existing database paths/queries were inspected:
usage tests use isolated temporary databases, timestamp indexes, bounded ranges
and limits, primary-key metadata lookup and transactional writes. Provider sync
full scans affect a two-row temporary fixture. No installed application database
was queried or modified by release verification.

Browser checks use fixtures and do not establish native Windows/WebView or real
account acceptance. No actual reset credits were consumed and no installed
Codex Tools application was replaced or restarted.

## Publication

Dispatch `release.yml` on the release commit with `release_tag=v3.1.1` and
`publish_npm=false`. Verify both validation jobs, three desktop platform builds,
release assets/signatures and public updater manifest before declaring success.
