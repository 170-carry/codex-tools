<div align="center">
  <h1>Codex Tools</h1>
  <p><strong>Codex 多账号管理、CLI/TUI 切换、用量查看和本地 OpenAI 兼容 API 反代工具。</strong></p>
  <p>
    <a href="https://github.com/170-carry/codex-tools/releases"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/170-carry/codex-tools?label=release"></a>
    <a href="https://www.npmjs.com/package/@170-carry/ctc"><img alt="npm" src="https://img.shields.io/npm/v/@170-carry/ctc?label=npm"></a>
    <a href="https://github.com/170-carry/codex-tools/blob/main/LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-blue"></a>
    <img alt="Platforms" src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows-lightgrey">
  </p>
</div>

## Overview

Codex Tools 面向同时使用多个 Codex 账号的场景，提供桌面 GUI、命令行入口和本地 `/v1` 反代能力。你可以用它导入账号、查看用量、切换本机 Codex 登录态，也可以把账号池暴露成 OpenAI 兼容接口供 Cursor、ChatWise、CC Switch、本地脚本等工具调用。

## Contents

- [Highlights](#highlights)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [CLI Reference](#cli-reference)
- [API Proxy](#api-proxy)
- [Development](#development)
- [Release](#release)
- [Documentation](#documentation)
- [Troubleshooting](#troubleshooting)

## Highlights

- 多账号管理：OAuth 登录导入、JSON 批量导入、账号备份回导入。
- 用量查看：展示 `5h`、`1week` 用量窗口和账号计划类型。
- 快速切换：切换 `~/.codex/auth.json` 和 `config.toml`，可联动启动 `codex app`。
- CLI/TUI：通过 `ctc` 执行 `list/switch/login/import/export/usage/doctor/report/tui`。
- API 反代：本地提供 OpenAI 兼容 `/v1` 接口，支持运行中账号轮换。
- App/CLI 绑定：可一键把 Codex App/CLI 切到本机反代地址，也可一键恢复原配置。
- 公网访问：集成 cloudflared，支持快速隧道和命名隧道。
- 桌面能力：状态栏驻留、自动更新、多语言界面、编辑器联动。

## Preview

![Codex Tools Screenshot](public/ScreenShot.png)

## Installation

### CLI/TUI

推荐通过 npm 安装命令行入口：

```bash
npm i -g @170-carry/ctc
```

安装后使用：

```bash
ctc list --json
ctc switch --best --launch
ctc tui
ctc ui
```

不想全局安装时，可以直接运行：

```bash
npx @170-carry/ctc list --json
```

`ctc ui` 会打开本机已安装的 Codex Tools 桌面应用；npm 包负责安装命令行入口和原生 CLI，不负责安装 `.app` 或 `.exe` 桌面包。

### Desktop App

桌面应用从 GitHub Releases 下载：

- [Latest Release](https://github.com/170-carry/codex-tools/releases/latest)
- macOS Apple Silicon / Intel
- Windows x64

安装桌面应用后，可以直接在 GUI 内完成账号导入、用量刷新、切换、API 反代和 cloudflared 配置。

## Quick Start

### Terminal

```bash
ctc login --label work
ctc list --refresh --json
ctc switch --best --launch
ctc doctor --json
```

常见流程：

1. 用 `ctc login` 调用官方 `codex login` 并导入账号。
2. 用 `ctc list --refresh` 查看账号和用量。
3. 用 `ctc switch 1` 或 `ctc switch --best` 切换账号。
4. 用 `ctc doctor` 检查本机环境和账号库状态。

### Desktop

1. 打开 Codex Tools。
2. 导入一个或多个 Codex 账号。
3. 刷新账号用量。
4. 选择账号并切换，或启动本地 API 反代。
5. 需要公网访问时，再开启 cloudflared。

## CLI Reference

所有命令默认读取桌面应用相同的数据目录。需要隔离环境时，加 `--data-dir <目录>`。

| 命令 | 说明 |
| --- | --- |
| `ctc list --json` | 列出已保存账号，输出 JSON |
| `ctc list --refresh --json` | 刷新用量后列出账号 |
| `ctc switch 1 --json` | 切换到第 1 个账号 |
| `ctc switch --best --launch` | 按余量选择更合适的账号，并启动 `codex app` |
| `ctc login --label work` | 调用官方 `codex login`，登录后自动导入账号 |
| `ctc import ./auth.json --json` | 导入账号 JSON |
| `ctc import ./accounts-dir --json` | 导入目录中的账号 JSON |
| `ctc import --current --json` | 导入当前 `~/.codex/auth.json` |
| `ctc export ./accounts.json --json` | 导出账号库 |
| `ctc export --json` | 直接把账号库 JSON 输出到终端 |
| `ctc usage --cached --json` | 查看本地缓存用量 |
| `ctc doctor --json` | 检查数据目录、Codex CLI、账号库和本机 auth 文件 |
| `ctc report --json` | 输出完整诊断报告 |
| `ctc tui` | 打开终端账号选择器 |
| `ctc ui` | 打开已安装的 Codex Tools 桌面应用 |

## API Proxy

Codex Tools 可以启动本地 OpenAI 兼容反代：

- 默认地址：`http://127.0.0.1:8787/v1`
- 鉴权方式：应用内生成的 `sk-...` API Key
- 上游来源：已导入的 Codex 账号
- 账号选择：按可用额度自动选择，支持运行中切换

更多链路说明见 [docs/api-proxy.md](docs/api-proxy.md)。

### Local Clients

本地脚本、`curl`、ChatWise 等本机直连客户端，可以直接使用本地 `Base URL`：

```bash
curl http://127.0.0.1:8787/v1/models \
  -H "Authorization: Bearer sk-..."
```

### Cursor

Cursor 可能由服务端代发请求，不建议填写 `127.0.0.1`、`localhost`、`192.168.x.x`、`10.x.x.x` 等本地或私网地址。

如果 Cursor 报 `ssrf_blocked` 或 `connection to private IP is blocked`，请改用：

- 应用内 cloudflared 生成的 `Public URL`
- 远程 Linux 反代地址
- 自己的公网域名反向代理地址

模型名称建议使用 `gpt-5.4`；同时兼容 `gpt-5-4` 别名。

### CC Switch

在 CC Switch 的 Codex 自定义 provider 中填写：

- `Base URL`：`http://127.0.0.1:8787/v1`
- `API Key`：应用内生成的 `sk-...`
- `wire_api`：`responses`

如果 Codex App/CLI 通过 wrapper、app bind、CC Switch 或自定义 provider 指向这个 `Base URL`，账号轮换会发生在本地反代层，不需要关闭 Codex App/CLI。

反代面板也提供“切到本机反代”和“恢复正常地址”按钮，会自动备份并恢复 `~/.codex/config.toml` 与 `~/.codex/auth.json`。

### Anthropic Messages

兼容 Anthropic Messages 的客户端可以请求：

- 地址：`http://127.0.0.1:8787/v1/messages`
- Key：`x-api-key: sk-...`
- 版本：`anthropic-version: 2023-06-01`

这里的 `2023-06-01` 是 Anthropic API version，不是模型版本日期。

## Development

### Requirements

- Node.js 20+
- Rust stable
- macOS 或 Windows

### Run Locally

```bash
npm install
npm run tauri dev
```

### Build CLI

```bash
cd src-tauri
cargo build --bin codex-tools-cli
```

### Build Frontend

```bash
npm run build
```

## Release

本仓库使用 GitHub Actions 自动发布桌面安装包和 npm CLI 包。

触发发布：

```bash
git tag v2.0.1
git push origin v2.0.1
```

发布内容：

- macOS Apple Silicon 桌面安装包
- macOS Intel 桌面安装包
- Windows x64 桌面安装包
- npm wrapper：`@170-carry/ctc`
- npm native packages：`@170-carry/ctc-darwin-arm64`、`@170-carry/ctc-darwin-x64`、`@170-carry/ctc-win32-x64`

npm 发布需要在 GitHub repository secrets 中配置 `NPM_TOKEN`。

## Documentation

- [How to Use](how%20to%20use.md)
- [API Proxy](docs/api-proxy.md)
- [Linux Proxyd](docs/linux-proxyd.md)
- [Changelog](changelog.md)

## Troubleshooting

### npm 安装后缺少原生包

如果安装时禁用了 optional dependencies，可能会看到缺少平台包的提示。重新安装：

```bash
npm i -g @170-carry/ctc --include=optional
```

### macOS 提示应用已损坏

如果 macOS 拦截未签名或隔离属性残留的应用，可以执行：

```bash
sudo spctl --master-disable
sudo xattr -r -d com.apple.quarantine /Applications/Codex\ Tools.app
```

### Cursor 无法访问本地地址

如果 Cursor 返回 `ssrf_blocked`，说明它的请求侧无法访问本机私网地址。请使用 cloudflared、远程 Linux 反代或公网域名。

## Project Layout

```text
src/                     React frontend
src-tauri/               Tauri and Rust backend
src-tauri/src/bin/       Native CLI binaries
npm/                     npm wrapper and platform packages
docs/                    Extra documentation
.github/workflows/       Release workflows
```

## Star History

<a href="https://www.star-history.com/?repos=170-carry/codex-tools&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/image?repos=170-carry/codex-tools&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/image?repos=170-carry/codex-tools&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/image?repos=170-carry/codex-tools&type=date&legend=top-left" />
 </picture>
</a>

## License

MIT，详见 [LICENSE](LICENSE)。
