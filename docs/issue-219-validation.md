# Issue #219: Codex model discovery and navigation

Validated locally on 2026-10-05. Source branch: `codex/fix-219-model-catalog-motion`.
This change has not been published as an installer or deployed to the reporter's machine.

## Reproduced cause

The desktop-bundled `codex-cli 0.159.0`, using API-key auth and an overridden
`openai_base_url`, returned seven bundled models from `model/list`, without
`gpt-6.1-sol`. A loopback `/v1/models` server advertised that model, but Codex
made no HTTP request to it. A fresh ChatGPT model cache did not change the result.
This explains why toggling the model in Codex Tools did not repair the desktop
picker.

An explicit [`model_catalog_json` configuration](https://developers.openai.com/codex/config-reference/)
returned eight supported text models, including `gpt-6.1-sol`, with its low
default reasoning effort. Discovery worked without network access. The
metadata retains each model's instructions and capabilities; the source and
Apache-2.0 license are under `src-tauri/src/proxy_service/catalog/`.

An empty explicit catalog caused this client to fall back to its bundled list.
When all models are unavailable, the generated catalog therefore contains a
hidden, API-ineligible metadata entry; the actual picker then returns zero models.
Request authorization continues to use the existing proxy policy checks.

## Resulting behavior

- Binding or selecting **同步模型目录** writes a separate managed catalog and
  configures Codex to load it. Restart Codex App/CLI to update its in-memory picker.
- Proxy startup and model/key policy edits synchronize an existing managed
  binding. The catalog intersects enabled models with that key's whitelist;
  disabled, revoked, and image-only keys expose no selectable text models.
- Existing configuration/auth backups remain the restore source. Switching
  accounts removes the managed catalog setting; unrelated catalog paths are
  preserved. Rebinding to another port updates routing metadata without
  replacing the original backup.
- Catalog parsing is cached; unchanged catalogs/configuration are not rewritten.
  Status polling does not generate catalogs. No SQL or schema changes were added.
- Lazy view navigation retains the current page while the next chunk loads.
  Pointer approach and keyboard focus preload the intended view. Classic layout
  adds a sliding navigation indicator and a brief fade, respecting reduced motion.

## Validation

| Check | Result |
| --- | --- |
| Real Codex app-server: legacy, fresh binding, stale cache, restricted key, all disabled | 5 scenarios passed; legacy failure reproduced |
| Scoped Rust profile/catalog/Sol/Astra regression tests | 35 distinct tests passed |
| Frontend unit tests, using Node 24.19.0 | 37 passed |
| Page/layout render checks | 73 passed |
| Existing browser interaction suite | 61 passed; zero runtime errors |
| New navigation and sync interactions, both layouts | 16 passed; zero runtime errors |
| Production frontend build and ESLint | Passed |
| Headless proxy crate `cargo check --locked` | Passed |
| `git diff --check` | Passed |

The browser suite uses the existing mocked native backend. The new navigation
suite holds the destination module request open to verify that the old page
stays visible, then releases it and checks the completed navigation. Classic
indicator alignment, a 760px window, reduced motion, and the bound sync button
were also checked. Screenshots were visually inspected.

The Rust file tests use temporary directories. The real Codex probe uses a
temporary client home, dummy API key and loopback server. It does not read user
credentials/history or send inference requests. These checks do not establish
Windows device acceptance or a successful inference call with a live account.

## Reproduction commands and evidence

```sh
python3 scripts/check-codex-model-discovery.py --codex /path/to/codex
cargo test --manifest-path src-tauri/Cargo.toml --lib profile_files
cargo test --manifest-path src-tauri/Cargo.toml --lib codex_catalog
cargo test --manifest-path src-tauri/Cargo.toml --lib sol_6_1
cargo test --manifest-path src-tauri/Cargo.toml --lib astra_tests
cargo check --manifest-path src-tauri/proxyd/Cargo.toml --locked
node --test tests/*.test.ts  # Node 22.18+ / 24 with native TypeScript support
pnpm test:pages
pnpm build
pnpm lint
```

For browser checks, start Vite on `127.0.0.1:5199`, set
`CODEX_TOOLS_PLAYWRIGHT_MODULE` to an installed Playwright module if needed,
then run `node scripts/check-ui.mjs` and `node scripts/check-navigation.mjs`.
The general UI suite now clicks the visible switch label and waits for lazy
navigation to commit before checking the destination's sections.

Local run evidence is in `/tmp/codex-tools-issue219/`: `model-discovery.json`,
`*-tests.log`, `proxyd-check.log`, `ui/ui-checks.json`,
`navigation/results.json`, and screenshots under `ui/` and `navigation/`.
