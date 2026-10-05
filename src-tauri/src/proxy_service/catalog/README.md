# Codex model metadata

`codex-models.json` is derived from OpenAI Codex's Apache-2.0 licensed
[`codex-rs/models-manager/models.json`](https://github.com/openai/codex/blob/main/codex-rs/models-manager/models.json),
Git blob `77e0389c56000ca19df5029278c30c3e9528af51`, retrieved on 2026-10-05.
The upstream license is included in `LICENSE`.

Modifications: retain only text models supported by this proxy. At runtime,
mirror each model's `model_messages.instructions_template` into the legacy
`base_instructions` field for older clients, avoiding duplicate embedded data. Keep instructions and capability
metadata together; inventing a minimal picker entry also changes the agent's
tools, prompt, reasoning options, and context window.

API-key Codex clients with an overridden OpenAI base URL can ignore both
`/v1/models` and their ChatGPT model cache. The explicit `model_catalog_json`
setting makes discovery independent of that cache and works offline. The
binding code filters this seed by the proxy's model switches and the bound
key's whitelist. Image-only API models are intentionally absent.

When adding a supported Codex model, update this metadata from upstream too,
retain its license, and run `scripts/check-codex-model-discovery.py` against the
desktop-bundled Codex binary. This catalog describes proxy compatibility, not
the upstream account's model entitlements.
