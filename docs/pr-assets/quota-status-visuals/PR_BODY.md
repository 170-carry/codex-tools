<!-- PR title / PR 标题: [EN/ZH] Add cross-platform quota status visuals and a Windows taskbar widget / 添加跨平台额度状态视觉与 Windows 任务栏组件 -->

> **Draft status / 草稿状态:** The macOS and Windows changes, real-machine evidence, and authorization hardening are synchronized in one branch and have passed final combined validation. Local release builds generated the application and platform installers, but updater metadata still requires the repository's `TAURI_SIGNING_PRIVATE_KEY`. / macOS 与 Windows 修改、实机证据及授权加固已同步到同一分支，并通过最终合并验证。本地发布构建已生成应用与平台安装包，但更新元数据仍需仓库的 `TAURI_SIGNING_PRIVATE_KEY` 才能完成签名。

## Summary / 摘要

- **Add consistent quota visuals across desktop surfaces / 为桌面额度展示提供一致视觉:** share five quota-icon styles across Windows and macOS, including two variants inside the logo-and-progress-ring style.
- **Add a native Windows taskbar quota widget / 新增 Windows 原生任务栏额度组件:** place quota information on the left or right side of the taskbar, or keep it in the system tray, with live settings updates.
- **Add a macOS first-launch quota setup / 新增 macOS 首次启动额度设置:** let users configure classic status text and the compact quota icon independently before entering the main application.
- **Harden recovery and isolated development / 加固恢复逻辑与隔离开发:** recover taskbar surfaces after Explorer changes, preserve cached error states, and safely adopt newer authorization snapshots without cloning production credentials into a fresh preview environment.

## Changes / 改动

### 1. Shared quota visual styles and settings / 共享额度视觉样式与设置

The menu bar and taskbar surfaces now support gradient number plate, gradient number card, gradient number, number with progress bar, and logo with progress ring. The logo-and-ring style contains ring-only and ring-with-exact-percentage variants. The same stored configuration drives supported Windows and macOS surfaces, and changes apply immediately.

菜单栏与任务栏展示现在支持渐变数字方牌、渐变数字横牌、渐变数字、数字加进度条，以及图标加进度环；图标加进度环同时提供“仅进度环”和“显示精确百分比”两个子方案。Windows 与 macOS 的受支持展示面共用同一套持久化配置，修改后立即生效。

**macOS menu-bar quota effect / macOS 菜单栏额度效果**

![macOS menu-bar quota effect / macOS 菜单栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-menu-bar-quota-effect.png)

### 2. Native Windows taskbar and tray quota surfaces / Windows 原生任务栏与托盘额度展示

Windows can render quota information as a native taskbar child surface on either side of the taskbar or as a system-tray icon. The implementation handles Windows Widgets placement, taskbar auto-hide, fullscreen windows, DPI and Explorer recreation, uses premultiplied transparency, and rate-limits UI Automation scans.

Windows 可将额度作为原生任务栏子组件放在任务栏左侧或右侧，也可以仅使用系统托盘图标。实现覆盖 Windows Widgets 位置、任务栏自动隐藏、全屏窗口、DPI 与 Explorer 重建，采用预乘透明渲染，并限制 UI Automation 扫描频率。

**Windows taskbar quota effect / Windows 任务栏额度效果**

![Windows taskbar quota effect / Windows 任务栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-taskbar-quota-effect.png)

### 3. macOS first-launch quota setup / macOS 首次启动额度设置

The macOS onboarding dialog configures classic status text and the compact quota icon independently. Either surface can be enabled or disabled, both can remain disabled, all visual choices update live, and completion is persisted separately from the Windows onboarding state. The dialog intentionally contains no simulated menu-bar screenshots or detached preview copy.

macOS 首次设置对话框可分别配置经典文字额度栏和紧凑额度图标。两者都能独立启用或关闭，也允许同时关闭；全部视觉选择都会实时更新，完成状态与 Windows 引导分别持久化。界面不再包含模拟菜单栏截图或独立的预览说明文字。

**macOS first-launch quota setup effect / macOS 首次启动额度设置效果**

![macOS first-launch quota setup effect / macOS 首次启动额度设置效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-first-launch-quota-setup.png)

### 4. Runtime recovery and authorization freshness / 运行时恢复与授权新鲜度

Quota surfaces retain explicit fresh, stale, and error states instead of silently clearing a previous refresh failure. Windows recreates its taskbar surface after Explorer or taskbar changes. When the active instance receives a newer, rotated authorization snapshot whose refresh timestamp is not older than the stored snapshot, it can clear the obsolete authorization block and resume quota refresh; the same stale snapshot is never treated as recovery.

额度展示会明确保留正常、缓存过期和错误状态，不再因普通设置刷新而静默清除此前的额度错误。Windows 会在 Explorer 或任务栏变化后重新创建任务栏展示。当当前实例获得刷新令牌已轮换、且刷新时间不早于已保存快照的新授权时，可以清除旧授权阻塞并恢复额度刷新；相同的旧快照不会被误判为已经恢复。

On macOS, status data now refreshes every 60 seconds even when both visible quota surfaces are disabled. Scheduled refresh sources reuse a successful result for up to 25 seconds, while startup, manual, import, and frontend-triggered refreshes always execute normally.

在 macOS 上，即使两个可见额度入口都关闭，状态数据仍每 60 秒刷新一次。定时刷新来源可复用 25 秒内的成功结果，而启动、手动、导入和前端触发的刷新仍会正常执行。

Fresh isolated previews no longer copy the production account store or production `auth.json`. Existing preview data is preserved, so developers can migrate or reauthorize intentionally without destructive startup behavior.

新的隔离预览不再复制正式版账号库或正式版 `auth.json`。已有预览数据保持不变，开发者可以主动迁移或重新授权，不会在启动时发生破坏性覆盖。

### 5. Localization, documentation, and compatibility / 本地化、文档与兼容性

- Add matching English, Simplified Chinese, Japanese, Korean, and Russian strings. / 补齐英文、简体中文、日文、韩文和俄文文案。
- Document Windows taskbar placement, first-run behavior, and the Windows Widgets interaction. / 说明 Windows 任务栏位置、首次运行行为以及与 Windows Widgets 的交互。
- Preserve existing account, API proxy, analytics, editor integration, and non-Windows behavior. / 保持现有账号、API 反代、分析、编辑器联动及非 Windows 行为。

## Validation / 验证

### macOS Apple Silicon

- [x] Frontend production build and TypeScript / 前端生产构建与 TypeScript：`npm run build`
- [x] ESLint completed with zero errors and four baseline unused-disable warnings / ESLint 完成，0 个错误、4 个基线 unused-disable 警告：`npm run lint`
- [x] Onboarding tests / 首次设置测试：7 passed
- [x] Usage-error tests / 额度错误测试：8 passed
- [x] Full Rust suite / 完整 Rust 测试：225 passed
- [x] Rust formatting / Rust 格式：`cargo fmt --check`
- [x] Clippy completed successfully; macOS reports cross-platform dead-code warnings for Windows-only rendering helpers / Clippy 成功完成；macOS 对 Windows 专用渲染辅助代码报告跨平台 dead-code 警告
- [x] Debug app and DMG generated; the app launched with isolated data / 已生成 debug 应用与 DMG，并使用隔离数据启动应用
- [x] Computer Use regression: live style updates, both quota surfaces disabled, completion persistence, and restart behavior / Computer Use 回归：视觉样式实时更新、两类额度展示同时关闭、完成状态持久化及重启行为
- [x] Runtime scheduling regression: 60-second background status refresh and 25-second scheduled-result reuse / 运行时调度回归：60 秒后台状态刷新与 25 秒定时结果复用
- [x] Five locale files have identical key sets / 五种语言文件的键集合完全一致
- [x] Production dependency audit / 生产依赖审计：0 vulnerabilities

### Windows 11 on ROG Strix

- [x] Frontend dependency preflight, production build, TypeScript, and ESLint / 前端依赖预检、生产构建、TypeScript 与 ESLint
- [x] Node regression tests / Node 回归测试：15 passed
- [x] Full Rust suite / 完整 Rust 测试：233 passed
- [x] Rust formatting, Clippy, and diff whitespace checks / Rust 格式、Clippy 与差异空白检查：passed; Clippy reported 0 errors and 60 existing warnings / 通过；Clippy 报告 0 个错误和 60 个既有警告
- [x] Windows release application, MSI, and NSIS packages generated / 已生成 Windows release 应用、MSI 与 NSIS 安装包
- [x] Release executable startup smoke / release 可执行文件启动冒烟：passed; no account operations were performed / 通过；未执行账号操作
- [x] Taskbar left, taskbar right, hidden state, five tray styles, and live settings updates / 任务栏左侧、任务栏右侧、隐藏状态、五种托盘样式及设置实时更新
- [x] Settings-page regression: both quota surfaces can be disabled and re-enabled independently / 设置页回归：两个额度入口可独立关闭、同时关闭并重新启用
- [x] Explorer recreation and taskbar reattachment / Explorer 重建及任务栏重新附着：`CodexToolsTaskbarQuotaWidget` recreated and visible / `CodexToolsTaskbarQuotaWidget` 已重建且可见
- [x] Five targeted authorization-snapshot self-heal tests / 五项授权快照自愈定向测试：passed
- [x] `TaskbarDa` remained read-only at `0`; Widgets being disabled did not prevent the quota widget from working or recovering / `TaskbarDa` 全程只读并保持为 `0`；Widgets 关闭不影响额度组件工作或恢复
- [ ] The Windows Widgets enabled-state layout was not exercised because the test did not modify `TaskbarDa`. / 未验证 Windows Widgets 启用状态下的布局，因为本次测试没有修改 `TaskbarDa`。
- [ ] MSI/NSIS overwrite installation was not run because the packages use the same product identity and version as the installed application. / 未执行 MSI/NSIS 覆盖安装，因为安装包与已安装应用使用相同产品身份和版本。

## Known non-blocking baseline / 已知非阻断基线

- The minified frontend entry chunk is approximately 693 kB and still triggers Vite's existing 500 kB recommendation. / 压缩后的前端入口约为 693 kB，仍会触发 Vite 既有的 500 kB 拆包建议。
- The full development-dependency audit reports 9 advisories (2 low and 7 high) on both `origin/main` and this branch; production dependencies report 0. Patch-level lockfile upgrades are available but are outside this feature diff. / `origin/main` 与本分支的完整开发依赖审计均报告 9 个公告（2 个 low、7 个 high）；生产依赖为 0。可通过补丁级锁文件升级处理，但不属于本功能差异。
- Locally built macOS bundles are suitable for validation but are not Developer ID signed or notarized. / 本地构建的 macOS 包可用于验证，但未使用 Developer ID 签名或公证。
- The locally generated macOS updater archive and Windows updater metadata cannot be signed without `TAURI_SIGNING_PRIVATE_KEY`; platform applications and installers were generated before that signing step. / 本地无法在缺少 `TAURI_SIGNING_PRIVATE_KEY` 时签署 macOS 更新压缩包和 Windows 更新元数据；平台应用与安装包已在该签名步骤之前成功生成。

## Review notes / 审阅说明

- The screenshots use isolated, empty application data and contain no account identifiers or credentials. / 截图使用隔离的空白应用数据，不包含账号标识或凭据。
- The visual evidence shows only the new quota surfaces and first-launch UI. Because these are newly added features, no before/after comparison is included, and settings-page screenshots remain excluded. / 视觉证据仅展示新增的额度栏与首次启动界面。由于这些均为新增功能，因此不提供修改前后对比，也不包含设置页截图。
- All source changes and selected evidence files are synchronized in the final branch; the original ROG and macOS development worktrees remain preserved. / 所有源码修改与选定证据文件均已同步到最终分支；原 ROG 与 macOS 开发工作树保持不变。
