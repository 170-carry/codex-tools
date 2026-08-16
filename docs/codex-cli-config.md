# Codex CLI 配置机制（config-advanced 要点）

来源：https://developers.openai.com/codex/config-advanced

更详细的选项见该页与 Configuration Reference。本文只沉淀与本地多 profile 切换、项目配置、provider 相关的关键机制，作为 codex-tools 内部参考。

## 1. Profiles（命名配置层）

> Profiles let you save named configuration layers and switch between them from the CLI. When you pass `--profile profile-name`, Codex loads `~/.codex/config.toml`, then overlays `~/.codex/profile-name.config.toml`. Profile names can contain letters, numbers, hyphens, and underscores.

机制要点：

- `-p / --profile <name>` 的加载顺序：先 `~/.codex/config.toml`（base），再覆盖 `~/.codex/<name>.config.toml`（profile 层）。
- 每个 profile 一个独立 TOML 文件，文件名 `~/.codex/<name>.config.toml`，`name` 只能含字母、数字、连字符、下划线。
- profile 文件里用**顶层 key**，不要写成 `[profiles.name]` 表。

示例（`~/.codex/deep-review.config.toml`）：

```toml
model = "gpt-5.5"
model_reasoning_effort = "xhigh"
approval_policy = "on-request"
model_catalog_json = "/Users/me/.codex/model-catalogs/deep-review.json"
```

调用：

```bash
codex --profile deep-review
codex exec --profile deep-review "review this change"
```

profile 层位于 base 与 project/CLI 之间，只写与 base 不同的值即可。`model_catalog_json` 可在 profile 层覆盖，两处都设时取 profile 值。

### 版本分界（重要）

> In Codex 0.134.0 and later, `--profile` no longer reads `[profiles.profile-name]` from config.toml, and the top-level `profile = "profile-name"` selector is no longer supported.

- **≥ 0.134.0**：新机制，`-p fugu` 认 `~/.codex/fugu.config.toml`，不认主 `config.toml` 里的 `[profiles.fugu]` 或 `profile = "fugu"`。
- **< 0.134.0**：老机制，`-p fugu` 只认主 `config.toml` 里的 `[profiles.fugu]` 表，独立的 `fugu.config.toml` **完全无效，会报 `config profile not found`**。

排查 `not found` 第一步就是 `codex --version`，低于 0.134.0 必须升级或改回老写法。老机制迁移：把 `[profiles.name]` 的内容挪进 `~/.codex/name.config.toml`，并删掉主 `config.toml` 里的 `[profiles.name]` 表与 `profile = "name"` 选择器。

### 真实排查案例

`codex -p fugu` 报 `config profile `fugu` not found`，但 `~/.codex/fugu.config.toml` 存在、`CODEX_HOME` 空、主 `config.toml` 无 legacy 残留。根因是终端实际用的 codex 是 `0.130.0`（低于 0.134.0），不认独立 profile 文件。同一机器另一处装的 `0.142.5` 正常。教训：`which -a codex` 可能列多个二进制，`which codex`（不带 `-a`）才是当前生效的那个，跨终端版本需各自确认。

## 2. 一次性 CLI 覆盖

> In addition to editing `~/.codex/config.toml`, you can override configuration for a single run from the CLI.

```bash
# 专用 flag
codex --model gpt-5.4

# 通用 key=value 覆盖（value 是 TOML，不是 JSON）
codex --config model='"gpt-5.4"'
codex --config sandbox_workspace_write.network_access=true
codex --config 'shell_environment_policy.include_only=["PATH","HOME"]'
```

- 优先用专用 flag（如 `--model`），需要覆盖任意 key 时用 `-c / --config`。
- key 支持点号嵌套，如 `mcp_servers.context7.enabled=false`。
- `--config` 的值按 TOML 解析；拿不准就加引号防 shell 分词；无法解析为 TOML 时按字符串处理。

## 3. 配置与状态位置

> Codex stores its local state under `CODEX_HOME` (defaults to `~/.codex`).

`CODEX_HOME` 下常见文件：

- `config.toml`：本地配置
- `auth.json`：文件型凭证（或走系统 keychain/keyring）
- `history.jsonl`：会话历史（开启时）
- 日志、缓存等其它 per-user 状态

`CODEX_HOME` 未设置时默认 `~/.codex`。排查 profile 问题时确认该变量未被改到别处。

只想把内置 OpenAI provider 指向代理/路由/数据驻留项目，直接设 `openai_base_url`，不用新建 `model_providers`：

```toml
openai_base_url = "https://us.api.openai.com/v1"
```

## 4. 项目级配置（.codex/config.toml）

> In addition to your user config, Codex reads project-scoped overrides from `.codex/config.toml` files inside your repo.

- 从项目根到 cwd 逐层加载 `.codex/config.toml`，同 key 取最靠近 cwd 的文件。
- 仅当项目受信任时才加载项目 `.codex/` 层（含 config、hooks、rules）；未信任则忽略，但 user/system 层照常加载。
- 项目配置里的相对路径（如 `model_instructions_file`）以所在 `.codex/` 目录为基准解析。
- 项目配置**不能**覆盖以下 key（会被忽略并告警）：`openai_base_url`、`chatgpt_base_url`、`apps_mcp_product_sku`、`model_provider`、`model_providers`、`notify`、`profile`、`profiles`、`experimental_realtime_ws_base_url`、`otel`。
  → provider、通知、telemetry 必须放用户级 `~/.codex/config.toml`；profile 用 `-p` + `~/.codex/<name>.config.toml` 选。

## 5. Hooks

可以从 `hooks.json` 或 config.toml 内联 `[hooks]` 表加载，位置需在生效的 config 层旁。常用四处：

- `~/.codex/hooks.json`
- `~/.codex/config.toml`
- `<repo>/.codex/hooks.json`
- `<repo>/.codex/config.toml`

项目级 hooks 仅在项目受信任时加载，user 级 hooks 独立于项目信任。同一层同时有 `hooks.json` 和内联 `[hooks]` 时两者都加载并告警，建议一层只用一种。

内联 TOML hooks 事件结构与 `hooks.json` 一致：

```toml
[[hooks.PreToolUse]]
matcher = "^Bash$"

[[hooks.PreToolUse.hooks]]
type = "command"
command = '/usr/bin/python3 "$(git rev-parse --show-toplevel)/.codex/hooks/pre_tool_use_policy.py"'
timeout = 30
statusMessage = "Checking Bash command"
```

## 6. 项目根探测

Codex 从 cwd 向上走，遇到项目根停止，用于发现 `.codex/` 层与 `AGENTS.md`。默认以含 `.git` 的目录作为项目根，可用 `project_root_markers` 自定义。

## 7. 排查清单

`codex -p <name>` 报 `not found` 时按序查：

1. `codex --version`：低于 0.134.0 是老机制，独立 `*.config.toml` 不生效。
2. `which codex`（不带 `-a`）：确认当前生效二进制，多版本注意区分。
3. `echo "$CODEX_HOME"`：空才用默认 `~/.codex`。
4. `ls -l ~/.codex/<name>.config.toml`：文件要存在。
5. `grep -nE '^\[profiles|^[[:space:]]*profile[[:space:]]*=' ~/.codex/config.toml`：≥0.134.0 时该输出应为空，有 legacy 残留需删。
