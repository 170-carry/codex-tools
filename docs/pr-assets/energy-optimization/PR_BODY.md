<!-- PR title / PR 标题: [EN/ZH] Improve the macOS status bar and reduce background energy use / 优化 macOS 状态栏并降低后台耗电 -->

## Summary

This PR improves the macOS status item and usage labels, significantly reduces background energy use, speeds up first-launch loading, and fixes the macOS status bar after a Plus-to-Pro upgrade. It also includes maintainer-facing release-note plumbing and debug-only redacted auth diagnostics.

Runtime quota percentages are still taken from the usage response. This change does not inject, estimate, or overwrite quota values; the `60%` values in `tray.rs` are isolated unit-test fixtures.

### What changed

- **Improve the macOS status bar and usage labels:** use the color app icon, default to one-week remaining usage, optionally show `5h / 1w` labels, hide the entire status item when disabled, keep the mode choices on the first row with the label toggle right-aligned below, and keep account meters labeled `5h / 1w` even when both values are currently identical.
- **Significantly reduce background energy use:** remove the inference keepalive request to `/backend-api/codex/responses`, delay and deduplicate refreshes, pause foreground polling while the main window is hidden, prevent overlapping token scans, and reuse unchanged token-log results.
- **Speed up first-launch loading:** render cached accounts first and start non-critical initialization concurrently.
- **Fix the macOS status bar after a Plus-to-Pro upgrade:** match by stable account identity before using the plan as variant metadata, so the status bar identifies the current account and displays its usage.

### Technical details

- **Broaden a narrow retry matcher:** explicit `provided authentication token is expired` and `token is expired` responses now use the existing refresh-and-retry path even when the status is not `401`. The `main` branch already retried `401 / unauthorized / invalid_token` responses and already avoided permanently blocking an account for access-token expiry.
- **Keep release descriptions optional and consistent:** when matching changelog notes exist, reuse them in the GitHub Release and Tauri updater metadata; otherwise emit a build warning, generate the legacy generic release text, and continue publishing normally.
- **Keep diagnostics out of release builds:** redacted auth parsing diagnostics remain behind `debug_assertions`.

## 摘要

本 PR 优化 macOS 状态栏和用量标签，大幅降低后台耗电，加快首次启动加载，并修复 Plus 升级为 PRO 后状态栏无法正确识别当前账号的问题。同时包含面向维护者的更新日志发布链路，以及仅调试版启用的脱敏授权诊断。

运行时额度百分比仍直接来自用量响应。本次改动不会注入、估算或覆盖额度数值；`tray.rs` 中的 `60%` 仅存在于隔离的单元测试样例中。

### 具体改动

- **优化 macOS 状态栏和用量标签：** 使用彩色应用图标，默认仅显示一周剩余用量，可选显示 `5h / 1w` 标签，选择“不显示”时隐藏整个状态项；模式选项位于第一行，标签开关在第二行右对齐，并在两个周期当前数值相同时仍分别标注 `5h / 1w`。
- **大幅降低后台耗电：** 移除向 `/backend-api/codex/responses` 发送的推理保活请求，延后并去重刷新任务，在主窗口隐藏时暂停前台轮询，阻止 Token 扫描重叠执行，并复用未变化的 Token 日志结果。
- **加快首次启动加载：** 优先渲染缓存账号，并发启动非关键初始化任务。
- **修复 Plus 升级为 PRO 后状态栏无法正确识别当前账号并显示用量的问题：** 先按稳定账号身份匹配，再把套餐作为变体元数据，使状态栏能够识别当前账号并显示其用量。

### 技术说明

- **扩展一个窄范围的重试匹配：** 明确包含 `provided authentication token is expired` 或 `token is expired` 的响应，即使状态码不是 `401`，现在也会复用已有的刷新并重试路径。`main` 原本已经会重试 `401 / unauthorized / invalid_token` 响应，也原本就不会因为 Access Token 过期而永久封锁账号。
- **保持发布说明可选且一致：** 存在匹配的更新说明时，将其复用于 GitHub Release 和 Tauri updater 元数据；不存在时仅输出构建警告，自动生成旧式通用发布文字并继续正常发布。
- **正式版不包含诊断行为：** 脱敏授权解析诊断仍限制在 `debug_assertions` 下。

## Before / After · 前后对比

### 1. macOS status-item appearance · macOS 状态项外观

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Incomplete monochrome status-item icon](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-before.png) | ![Color application icon in the macOS status bar](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-after.png) |

### 2. macOS status-item settings · macOS 状态项设置

**Before / 修改前**

![Status settings before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-before.png)

**After / 修改后**

![Status settings after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-after.png)

### 3. Usage-window labels · 用量周期标签

**Before / 修改前**

![The five-hour meter was mislabeled as 1w](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-before.png)

**After / 修改后**

![The semantic slots are labeled 5h and 1w](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-after.png)

### 4. Background work and startup · 后台任务与启动

![Background work before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/background-work-before-after.png)

_This diagram summarizes code-path changes; it is not a quantitative battery benchmark._

_该图概括代码路径变化，不代表定量耗电基准测试。_

### 5. Current-account resolution · 当前账号识别

![Current account resolution before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/account-resolution-before-after.png)

### 6. In-app release notes · 应用内更新说明

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Generic release notes before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/in-app-release-notes-before.png) | ![Actual localized release notes after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/in-app-release-notes-live-preview.jpg) |

## Release-note flow · 更新说明链路

`changelog.md` is now the single source of truth:

`changelog.md` 现在是唯一文案来源：

```text
changelog.md
  └─ scripts/extract-release-notes.mjs
       ├─ GitHub Release body / GitHub Release 正文
       └─ latest.json.notes
            └─ update.body
                 └─ localized in-app dialog / 本地化应用内弹窗
```

At release time, matching bilingual notes from `changelog.md` are reused by the GitHub Release and the in-app updater. Release notes remain optional: missing notes produce a warning and generated generic text, without blocking the build or release.

正式发布时，工作流会将 `changelog.md` 中匹配的双语说明复用于 GitHub Release 和应用内更新弹窗。更新说明仍是可选项：缺失时只会产生警告并使用自动生成的通用文字，不会阻断构建或发布。

## Validation · 验证

- [x] Frontend lint and production build: `npm run lint`, `npm run build`
- [x] Focused before/after crops use isolated demo UI or prior test captures and contain no directly identifying account data / 局部前后对比图来自隔离演示界面或既有测试截图，不包含可直接识别账号身份的数据
- [x] macOS status-item appearance and settings are both shown separately / macOS 状态项外观与设置界面均分别展示前后效果
- [x] Usage-label fixture sets both windows to 604800 seconds; the UI still renders the semantic `5h` and `1w` labels
- [x] Release-note extraction checked for an exact tag, Unreleased fallback, and a missing changelog; missing optional notes only warn and use generated text / 已验证精确版本、Unreleased 回退及缺少 changelog 三种情况；缺少可选说明时仅警告并使用自动生成文字
- [x] In-app updater preview checked against local `latest.json` metadata
- [x] Full Rust test suite: 174 passed
- [x] Release workflow YAML, SVG assets, and final `git diff --check`

## Scope notes · 范围说明

- The macOS status item remains a macOS-only feature; the account, startup, token-log, auth refresh, and release-note improvements are cross-platform where their underlying code paths are shared.
- No account export, token, email, or other direct account identifier is present in the screenshots. Cropped quota percentages are not connected to an account identity; preview apps used isolated bundle identifiers and empty demo stores.

- macOS 状态项仍是 macOS 专属功能；账号、启动、Token 日志、授权刷新与更新说明等改动会在共享代码路径上跨平台生效。
- 截图不包含账号导出、Token、邮箱或其他可直接识别账号身份的数据；局部额度百分比未关联账号身份，预览应用使用独立 Bundle ID 与空白演示数据。
