# v3.1.2 validation

Prepared on 2026-10-19 from the full local working set and merged PR #231.

## Scope

- Fix ChatGPT image generation through native Codex image endpoints.
- Use the classic account, analytics, proxy and settings interface throughout;
  remove the alternate layout and its saved-preference handling.
- Refine account details, plan labels, unavailable values and token statistics.
- Include the modular issue #229 diagnostic scripts and historical, redacted
  local compatibility report. These are diagnostics, not a Remote product switch;
  real-phone Remote acceptance remains outstanding. The scripts were syntax
  checked for this release; their live-account probes were not rerun.
- Set app, Tauri, Cargo and root package metadata to 3.1.2. Keep npm distribution
  package versions unchanged; publish desktop installers with npm disabled.

## Local validation

- 343 Rust library tests passed with TZ=UTC, including the 11 new image tests.
- ESLint passed; 30 frontend unit tests passed with Node 24.19.0.
- 33 page and 25 layout render checks passed. The synthetic PROMAX account adds
  a ninth account group; update the corresponding fixture-count assertions.
- 29 browser interaction checks passed with zero browser runtime errors,
  covering account actions, navigation, theme, empty states and minimum width.
- TypeScript/Vite production build passed; the existing bundle-size warning
  remains. Rust formatting passed.
- Packaged standalone proxy passed locked Cargo check, including the new native
  image transport source files in the generated installer resource tree.
- All five issue #229 Python modules passed syntax checks.
- Release-note extraction returned the three short bilingual v3.1.2 entries.
- Diff whitespace and new-file credential-literal checks passed. Historical
  credential-file hashes were removed from the public diagnostic report;
  before/after equality results remain.

## Data and acceptance

No new SQL or schema change. Existing Rust database tests use disposable local
fixtures: timestamp/key indexes and bounded event ranges, primary-key metadata
lookups, transactional writes, and a two-row provider-sync fixture. Query scope
and write impact were reviewed before execution. No installed account database
was used for validation. The Rust suite runs with TZ=UTC because an existing
fork-history fixture assumes its UTC date boundary.

Browser tests use synthetic accounts and mocked native commands. They do not
consume real reset credits or establish real-account/device acceptance.

## Publication

Dispatch release.yml on the release commit with release_tag=v3.1.2 and
publish_npm=false. Completion requires both validation jobs, all three desktop
builds, the full installer/signature set, and public latest.json verification.
