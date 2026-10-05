# v3.0.3 validation

Prepared on 2026-10-05. PR [#220](https://github.com/170-carry/codex-tools/pull/220)
was reviewed, reproduced and merged as `67d06a640c9c10e10bb8cad1e7c732148eaca57c`.
Issue #219's cause and native Codex discovery checks are documented in
[issue-219-validation.md](issue-219-validation.md).

## Measured changes

Chrome 154 on macOS, local Vite frontend, synthetic data and mocked native
commands. These timings do not measure upstream network latency, native WebKit
frame rate, system-wide power or production database performance.

| Workload | Before / uncached | After / cached |
| --- | ---: | ---: |
| Focus 36 visible Settings controls: maximum outer-shell scroll | 217 px | 0 px |
| 1,000 classic date-format pairs, median of 5 runs | 32.5 ms | 1.2 ms |
| 1,000 compact quota-time formats, median of 5 runs | 23.0 ms | 2.1 ms |
| 1,000 analytics formatting batches, median of 5 runs | 33.4 ms | 1.1 ms |
| Restore 100 classic account rows, 5 alternating samples per mode | 46.1 ms | 36.3 ms |
| Restore 100 compact account rows, 5 alternating samples per mode | 23.5 ms | 12.9 ms |
| Startup calls for accounts, token totals, proxy status and tunnel status | Twice each | Once each |

Separate before/after list runs showed timing noise, especially in classic
layout. The final comparison alternated cached and uncached formatter behavior
on the same build, retaining identical DOM, synthetic accounts and startup
logic. It measures event-to-DOM-commit time, not time to painted pixels.

The formatter helper keeps at most 32 rule objects per type and clears on window
focus to pick up OS locale/time-zone changes. It never caches account values.
No new dependencies, worker framework, virtualized-list layer, or scheduling
system was introduced.

Dependency traversal, including type-only and dynamic imports, found six
unreachable legacy components. Repository/test reference checks confirmed that
they could be removed: `AccountCard`, `AddAccountSection`, `BottomDock`,
`DebugFloatingTool`, `MetaStrip`, and `FeatureSection` (734 lines). These were
already excluded from the production module graph; removal is maintenance
cleanup, not a claimed runtime speedup. Classic replacements remain in use.

Embedded model data shrank from 540,637 to 383,797 bytes by generating its legacy
instruction field once from the canonical instruction template. All metadata,
instructions, permissions and older-client compatibility are retained.

## Storage and background work

SQL was reviewed for indexes and scan bounds before execution. No user database
was queried or changed. A new in-memory SQLite database with 100,000 rows was
used; inserts were bounded to one transaction and query plans were inspected
before timing read-only queries.

- Latest 100 key logs: timestamp index scan with LIMIT, median 0.038 ms in the
  all-keyed fixture. Sparse legacy anonymous logs can require a longer scan;
  this result is not a worst-case production bound.
- One-hour statistics: timestamp range index, median 0.081 ms, 121 output rows.
- Thirty-day statistics: timestamp range index plus temporary grouping B-tree,
  median 40.34 ms, 620 output rows, at most 86,401 input rows in this fixture.
- Existing serialized usage writer, bounded query ranges, cached analytics,
  and visible-window polling gates remain. No SQL or schema changes were needed.

## Validation and publication

- Complete Rust library suite: 322 tests passed.
- Frontend unit suite: 40 tests (including locale/DST/invalid-date coverage).
- Static rendering: 43 page checks plus 30 layout checks passed.
- Browser regression: 61 checks passed, zero runtime errors.
- Navigation/model-sync browser regression: 16 checks passed, zero runtime errors.
- Actual desktop-bundled Codex 0.159.0: five discovery scenarios passed, without
  real credentials or inference requests.
- TypeScript/Vite production build, ESLint, Rust formatting, and packaged
  `src-tauri/gen/remote-build/proxyd` locked compilation passed.
- The frontend still has its existing main-chunk size warning. No unsupported
  bundle-size or whole-machine energy improvement is claimed.

`scripts/check-performance.mjs` repeats the alternating comparison and asserts
one-time startup reads and stable outer-shell focus. Use the existing preview
server and `CODEX_TOOLS_PLAYWRIGHT_MODULE` like the other browser checks.
Local evidence is under `/tmp/codex-tools-release-3.0.3/`, including JSON samples,
SQL plans, native test logs, browser results and screenshots.

Desktop installers/updater assets are published by the GitHub tag workflow
after macOS/Windows validation. npm publication is a separate pipeline: the
local machine has no npm login, and registry credentials must be verified by
the existing workflow. Do not infer npm success from desktop publication.
