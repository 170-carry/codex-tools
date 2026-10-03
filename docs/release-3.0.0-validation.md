# v3.0.0 release validation

Prepared on 2026-10-03. The complete local UI workspace was committed at `dbd90cf`, then merged with `origin/main` at `b977b52` so the v2.10.0 maintenance changes and GPT-6.1 Sol support are included.

The release contains the compact account table, inspectors, import dialogs, analytics/proxy/settings sections, keyboard and focus behavior, theme handling, small-window rules, screenshots, fixtures and review scripts. New modules are split by responsibility. Account rows preserve immutable import order; the saved API-account editor is connected to both account menus and the inspector and uses the common modal frame.

## Local checks

- Node regression suite: 33 passed, including quota reset dates, time-zone offsets and stable account order.
- React page rendering: 36 checks passed, including all five locales, draft retention, platform-specific quota controls and account order after refresh/switching. These are static rendering checks.
- ESLint: zero errors and zero warnings.
- TypeScript and production frontend build passed. The main frontend chunk still produces Vite's existing size warning; secondary pages load as separate chunks.
- The frontend compiler is pinned to esbuild 0.27.3. Clean installation of 0.27.7 failed to transform destructuring for the configured Safari 13 target, so its failed release checks published no installers. The pinned dependency passed Node tests, page rendering, lint and production build again.
- Rust library: 314 tests passed.
- Locked standalone proxy compilation passed from `src-tauri/gen/remote-build`, the actual installer resource tree.

## Browser checks

The actual frontend was exercised in the Codex in-app browser using the preview backend and example accounts. No real account was switched and no real credentials or inference requests were used.

- Five account columns, eight grouped rows for nine profiles, full reset dates and correct unknown/reset-credit states were visible.
- Saved Relay editing: masked/blank API key, changed URL and label, saved row, close and reopen. The current account remained unchanged during editing.
- Account switching through the preview backend preserved row order and one current row.
- Analytics session search and an unsaved proxy-key label survived section changes; only one section panel was visible.
- The model picker included `gpt-6.1-sol` and the other supported models.
- Light/dark themes, the 640 × 500 minimum viewport, footer visibility and import-dialog bounds were checked.
- Windows-layout OpenCode enable/restart/disable and settings reopen were checked with mocked native commands. This does not establish resolution of the reporter's Windows/WebView issue #206.
- No browser console errors were reported during these paths.

The new account workspace screenshot uses example data: `docs/pr-assets/compact-macos/v3-accounts-light.jpg`.

## Database and release boundaries

No SQL or database migrations were added by this release preparation. Existing Rust SQLite regression tests use small temporary fixtures; their schemas, index-backed time/key queries and bounded row counts were reviewed before execution. No user database was queried or modified.

The GitHub workflow validates macOS and Windows before building installers, updater archives and signatures. npm publication now checks the existing registry identity before compiling CLI packages. The v2.10.0 npm jobs returned E404, while their desktop installers succeeded; npm authentication/publication status must be reported separately from desktop release status.
