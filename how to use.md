# How to Use

这份文档提供一个简洁的使用流程，适合第一次使用 `codex-tools` 时快速上手。

## 1. 使用前准备

- 安装并打开 `Codex Tools`
- 准备好一个或多个 Codex 账号
- 如果要使用编辑器联动或 API 反代，先确保本机已经安装对应工具

## 2. 导入账号

应用启动后，先把账号导入进来。支持以下几种方式：

- OAuth 登录导入
- 上传单个或多个 `.json` 账号文件，或导出的 `accounts.json` 备份
- 直接选择一个文件夹，批量读取其中的账号文件

导入完成后，应用会保留你当前机器上的登录状态，不会直接覆盖正在使用的账号。

## 3. 查看账号用量

导入后，先刷新一次账号列表和用量信息。

你可以在界面中看到：

- 每个账号的 `5h` 用量
- 每个账号的 `1week` 用量
- 当前账号计划类型

这一页主要用来判断哪个账号还有可用额度。

## 4. 切换账号并启动 Codex

确认目标账号后，可以直接执行切换。

常见流程是：

1. 在账号列表中选择一个可用账号
2. 点击切换或启动按钮
3. 等待应用把本机环境切换到对应账号
4. 如果本机装了编辑器，也可以按需触发联动重启

如果找不到桌面应用，程序会自动尝试回退到 `codex app`。

## 5. 可选：开启 API 反代

如果你希望让其他工具通过 OpenAI 兼容接口接入，可以开启本地 API 反代。

基本流程：

1. 打开应用中的 API 反代面板
2. 选择或确认端口
3. 启动反代服务
4. 复制界面里显示的 `Base URL` 和 `API Key`
5. 在你的客户端中填入这些参数

启动后，本地会提供 `/v1` 兼容接口，并自动从账号池里选择可用账号转发请求。

注意：

- 本机直连客户端可以直接使用本地 `Base URL`
- Codex App/CLI 可以在反代面板点击“切到本机反代”，需要恢复时点击“恢复正常地址”
- 如果 Codex App/CLI 通过 wrapper、app bind、CC Switch 或自定义 provider 指向这个 `Base URL`，运行中账号轮换在反代层完成，不需要关闭 Codex App/CLI
- 负载均衡选择“逐个”时，带有稳定 session key 的同一会话会优先复用同一账号，额度或鉴权失败后再切到下一个账号
- `Cursor` 这类可能由服务端代发请求的客户端，不一定允许访问 `127.0.0.1` 或私网地址
- 如果 Cursor 报 `ssrf_blocked`，请改用 `cloudflared` 暴露出来的公网地址，或使用远程 Linux 反代

详细说明可参考 [docs/api-proxy.md](docs/api-proxy.md)。

如果你要通过 CC Switch 管理 Codex provider，也可以直接接本工具的反代：

- 在 CC Switch 的 **Codex 自定义 provider** 中填写本工具显示的 `Base URL`
- `Base URL` 填到 `/v1` 为止，例如 `http://127.0.0.1:8787/v1`
- `API Key` 使用本工具生成的代理 `sk-...`
- `wire_api` 选择 `responses`

如果你要接 Anthropic Messages 兼容客户端，可以直接请求：

- 地址：`http://127.0.0.1:8787/v1/messages`
- Key：`x-api-key: sk-...`
- 版本：`anthropic-version: 2023-06-01`

这里的 `2023-06-01` 是 Anthropic API version，不是模型版本日期。

更完整的配置示例见 [docs/api-proxy.md](docs/api-proxy.md) 里的“通过 CC Switch 接入 Codex”。

## 6. 可选：使用 CLI/TUI 管理账号

如果你要在终端里管理账号，或想把账号切换、导入导出、诊断接进脚本，可以使用 `ctc`。

安装：

```bash
npm i -g @170-carry/ctc
```

安装后执行：

```bash
ctc list --json
```

不想全局安装时，也可以直接运行：

```bash
npx @170-carry/ctc list --json
```

常用命令：

| 命令 | 用途 |
| --- | --- |
| `ctc list --json` | 列出账号，输出 JSON |
| `ctc list --refresh --json` | 刷新用量后列出账号 |
| `ctc switch 1 --json` | 切换到第 1 个账号 |
| `ctc switch --best --launch` | 自动选择余量更合适的账号，并启动 `codex app` |
| `ctc login --label work` | 调用官方 `codex login`，登录后导入账号 |
| `ctc import ./auth.json --json` | 导入账号 JSON |
| `ctc import ./accounts-dir --json` | 导入目录里的账号 JSON |
| `ctc import --current --json` | 导入当前 `~/.codex/auth.json` |
| `ctc export ./accounts.json --json` | 导出账号库 |
| `ctc export --json` | 直接把账号库 JSON 输出到终端 |
| `ctc usage --cached --json` | 查看本地缓存用量 |
| `ctc doctor --json` | 检查本地环境和账号库 |
| `ctc report --json` | 输出完整诊断报告 |
| `ctc tui` | 打开终端选择器，选择账号后切换 |
| `ctc ui` | 打开已安装的 Codex Tools 桌面应用 |

注意：

- `switch` 会写入本机 `~/.codex/auth.json` 和 `~/.codex/config.toml`
- `--json` 适合脚本读取，不需要解析普通文本
- `--data-dir <目录>` 可以指定账号库位置，适合测试、备份和多环境切换
- `login` 会直接运行官方 `codex login`，需要在可交互终端里使用
- `tui` 也需要在可交互终端里使用
- `ui` 需要本机已经安装 Codex Tools 桌面应用；npm 包负责打开应用，不负责安装 `.app` 或 `.exe`

## 7. 日常使用建议

- 新增账号后先刷新一次用量
- 在调用失败或额度不足时，优先切换到余量更高的账号
- 如果长期需要外部工具接入，可以保持 API 反代开启
- 如果要暴露给外网使用，可以再配置 `cloudflared`

## 8. 一句话总结

打开应用 -> 导入账号 -> 刷新用量 -> 选择账号 -> 切换并启动 -> 按需开启 API 反代；需要脚本化时使用 CLI/TUI。
