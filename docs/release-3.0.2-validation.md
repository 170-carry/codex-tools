# v3.0.2 release validation

Prepared on 2026-10-04 against `e09dde3` (v3.0.1). This release restores the original layout as the default and keeps the compact layout available from the title bar and Settings → General. The stored layout preference is independent of account data and of theme/settings persistence. The initial window returns to 1586 × 992.

Classic account presentation was recovered from the source preceding the compact redesign and split into small components. It reuses the current account controller, stable import order, authentication locks, API-account editor and proxy-participation behavior. Analytics, API proxy and settings each support both layouts. Compact CSS and theme variables are scoped to the compact mode so they cannot override the original appearance.

## Local validation

- 37 Node tests passed, including default-original behavior, both saved preferences, invalid preference fallback and unavailable local storage.
- 43 existing page-render checks and 30 layout-render checks passed, including both page structures, visible account controls, language choices and CSS isolation.
- Frontend TypeScript/build and ESLint passed. Vite retains its main-chunk size warning.
- Rust library: 314 tests passed with `cargo test --locked --lib`.
- Packaged standalone proxy: `cargo check --locked --manifest-path gen/remote-build/proxyd/Cargo.toml --target-dir proxyd/target` passed.
- Root npm package/lock, Tauri manifest/config/lock, standalone proxy manifest/lock, generated proxy build resource and all npm platform/wrapper versions agree on 3.0.2.
- Bilingual release-note extraction selected the explicit v3.0.2 entry.

These are code, static-render and build checks. Actual-window clicks, persistence across a native application restart and screenshots of the restored layout have not been revalidated in this run. The earlier automated browser access restriction remains a verification boundary.

## Data and publication boundaries

No SQL or database changes were added. Before the required Rust suite, existing SQL tests were reviewed: proxy usage and provider-sync tests use unique temporary directories, small fixture tables and bounded/index-backed reads; their full-table clear/migration operations affect only those fixtures. No live account database was queried or changed.

The configured GitHub tag workflow validates macOS and Windows, then produces macOS Apple Silicon, macOS Intel and Windows desktop installers, updater archives/signatures and latest.json. Public release assets and the updater manifest must be read back after publication.

The preceding v3.0.1 desktop build jobs succeeded, while npm publication failed at registry identity verification with E401. npm currently reports 2.9.0 as latest. This release does not change repository credentials; desktop publication and npm publication must be reported independently.
