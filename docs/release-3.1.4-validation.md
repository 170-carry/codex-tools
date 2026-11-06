# v3.1.4 review and validation

Reviewed on 2026-11-07 against v3.1.3 (`fb00f7f`).

## Issue decisions

| Issue | Decision |
| --- | --- |
| #242 | Fix Responses passthrough termination in a shared HTTP SSE/WebSocket module. On read failure, premature EOF or malformed event data, emit one `response.failed` with `upstream_stream_interrupted`. Convert generic upstream errors using only a bounded identifier code and a fixed message. Preserve partial output, response ID when available, and existing terminal events; do not replay requests or synthesize success. |
| #239 | Allow incremental `additional_tools` items at their original input positions. Validate every tools array and hosted-tool restriction; only merge top-level tools into the initial catalog. Preserve normalization idempotence. |
| #241 | Add GPT-6.1 Terra, aliases, Lite routing, native Codex catalog metadata and its standard short-context price estimate. Match Terra before the legacy Sol alias. Preserve existing Key restrictions and model switches. |
| #238 | Add GPT-Image-2.5 Sunburst and Flare to model controls and image request validation, including dated snapshots. Native transport preserves the selected image model. |
| #237 | Explain existing per-account proxy participation, priority/sequential routing and quota fallback. Distinguish proxy request routing from changing the desktop login or resuming a task. Arbitrary provider monthly balances are not exposed by a uniform interface. Close without adding desktop automation. |
| #240 | Reported message identifies an upstream DeepSeek/gateway HTTP 413 and mentions another client. Explain starting a fresh short conversation, reducing request size, switching to a compatible provider or checking the upstream gateway limit. No reproducible Codex Tools defect established. |

## Validation

- Rust library: 367 tests passed with `TZ=UTC cargo test --locked --lib`.
- Wire regressions inspect actual HTTP SSE body bytes and local WebSocket frames,
  without passing output through the production SSE decoder. Cases cover read
  errors, EOF, generic upstream errors, malformed/truncated JSON, fragmentation,
  normal completion, other terminal statuses, trailing errors and missing final
  SSE delimiter. Exactly one terminal event is required.
- Image transport tests use a local mock upstream and verify the model reaching
  `/backend-api/codex/images/generations` for both 2.5 models.
- Frontend: 36 unit tests, 33 page-render and 25 layout-render checks passed;
  ESLint and production build passed using Node 22.23.4. Host Node 20 cannot run
  the repository's TypeScript tests; validation uses the CI Node 22 major.
- Real desktop-bundled Codex 0.167.0-alpha.3.2 model discovery passed for fresh
  binding, stale cache, restricted Key and all-disabled scenarios, using dummy
  auth and temporary storage. Both Sol and Terra are visible when allowed.
- Packaged standalone proxyd `cargo check --locked` passed.
- Rust formatting and diff whitespace checks passed. Existing dead-code and
  frontend bundle-size warnings remain.

No SQL changes. Before database tests, existing query shapes and fixtures were
reviewed: usage reads use timestamp/key indexes and bounded time ranges; writes
use transactions; metadata and per-thread updates use primary keys. Whole-table
counts, provider regrouping/updates and clearing are confined to tiny disposable
test fixtures in this validation. No installed account database is opened by
these checks. New stream fixtures contain no usage and never initialize storage.

No live model generation, real account switch or production credential access
is used for acceptance. Model availability still depends on upstream entitlements.
The interruption fix improves explicit failure reporting; it does not repair
the upstream network or make a disconnected downstream writable. `response.done`
is preserved; no unverified conversion to successful completion is introduced.

## Publication

Release through `release.yml` with `release_tag=v3.1.4` and `publish_npm=false`.
Verify both CI platforms, all three desktop builds, installer hashes, updater
signatures and the public updater manifest before closing the fixed issues.

## Sources

- [Incremental tool catalogs](https://developers.openai.com/api/docs/guides/tools-tool-search)
- [GPT-6.1 Terra and standard pricing](https://developers.openai.com/api/docs/models/gpt-6.1-terra)
- [Image model IDs and endpoints](https://developers.openai.com/api/docs/guides/image-generation)
- [Codex model catalog](https://github.com/openai/codex/blob/main/codex-rs/models-manager/models.json), Terra imported from blob `489ebe3f2483348641a16160ce8491dd077eabca`; existing model metadata retained.
