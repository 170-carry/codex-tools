<!-- PR title / PR 标题: [EN/ZH] Add cross-platform quota status visuals and a Windows taskbar widget / 添加跨平台额度状态视觉与 Windows 任务栏组件 -->

> **Draft status / 草稿状态:** The feature branch is based on the latest `origin/main` v2.6.0. macOS validation and the synchronized ROG-Strix Windows frontend and native taskbar regressions passed, and the required quota and first-launch visuals are included. Local updater metadata still requires the repository's `TAURI_SIGNING_PRIVATE_KEY`. / 当前功能分支已基于最新的 `origin/main` v2.6.0。macOS 验证与 ROG-Strix Windows 前端、原生任务栏同步回归均已通过，所需的额度效果与首次启动视觉素材也已补齐。本地更新元数据仍需仓库的 `TAURI_SIGNING_PRIVATE_KEY` 才能签名。

## Summary / 摘要

- **Add consistent quota visuals across desktop surfaces / 新增 MacOS&Windows 双端额度图标:** share five quota-icon styles across Windows and macOS, including two variants inside the logo-and-progress-ring style.
- **Add a native Windows taskbar quota widget / 新增 Windows 任务栏额度组件:** place quota information on the left or right side of the taskbar, or keep it in the system tray, with live settings updates and reliable restoration after re-enabling.
- **Add cross-platform first-launch quota setup / 新增新版本首次启动额度图标设置:** let macOS users configure the text quota surface and compact quota icon independently, and let Windows users choose taskbar placement and tray presentation before entering the main application.
- **Harden recovery and isolated development / 加固恢复逻辑与隔离开发:** recover taskbar surfaces after Explorer changes, preserve cached error states, and safely adopt newer authorization snapshots without cloning production credentials into a fresh preview environment.
- **Reduce macOS background work / 降低 macOS 后台开销:** use one native status refresh scheduler, deliver fresh results to the UI, and defer freshness-only account-store writes.

## Changes / 改动

### 1. Shared quota visual styles and settings / 共享额度视觉样式与设置

The menu bar and taskbar surfaces now support a square number card, wide number card, gradient number, number progress bar, and app-icon progress ring. The app-icon ring style contains ring-only and ring-with-percentage variants. The same stored configuration drives supported Windows and macOS surfaces, and changes apply immediately.

菜单栏与任务栏展示现在支持方形数字卡、横向数字卡、渐变数字、数字进度条和图标进度环；图标进度环同时提供“仅显示进度环”和“显示百分比”两个子方案。

**macOS quota icon style choices / macOS 额度图标样式选择**

![macOS quota icon style choices / macOS 额度图标样式选择](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-quota-icon-style-settings.png)

**Windows quota icon style choices / Windows 额度图标样式选择**

![Windows quota icon style choices / Windows 额度图标样式选择](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-quota-icon-style-settings.png)

**macOS menu-bar quota effect / macOS 菜单栏额度效果**

![macOS menu-bar quota effect / macOS 菜单栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-menu-bar-quota-effect.png)

**Windows tray quota icon effect / Windows 托盘额度图标效果**

![Windows tray quota icon effect / Windows 托盘额度图标效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-tray-quota-icon-effect.png)

### 2. Native Windows taskbar and tray quota surfaces / Windows 原生任务栏与托盘额度展示

Windows can render quota information as a native taskbar child surface on either side of the taskbar or as a system-tray icon. The implementation handles Windows Widgets placement, taskbar auto-hide, fullscreen windows, DPI and Explorer recreation, uses premultiplied transparency, and rate-limits UI Automation scans.

Windows 可将额度作为原生任务栏子组件放在任务栏左侧或右侧，也可以仅使用系统托盘图标。实现覆盖 Windows Widgets 位置、任务栏自动隐藏、全屏窗口、DPI 与 Explorer 重建，采用预乘透明渲染，并限制 UI Automation 扫描频率。

Disabling the taskbar component now keeps its layered child window attached to the taskbar. Re-enabling it refreshes the native surface so the quota pixels reliably return; selecting either placement in the first-launch dialog also re-enables a hidden component.

关闭任务栏组件时，现在会保留透明分层子窗口与任务栏的挂载关系。重新启用时会刷新原生表面，确保额度像素恢复显示；在首次启动界面选择任一任务栏位置，也会自动重新启用已隐藏的组件。

**Windows taskbar quota effect / Windows 任务栏额度效果**

![Windows taskbar quota effect / Windows 任务栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-taskbar-quota-effect.png)

### 3. Cross-platform first-launch quota setup / 应用首次启动额度图标设置

The macOS onboarding dialog configures classic status text and the compact quota icon independently. Either surface can be enabled or disabled, both can remain disabled, all visual choices update live, and completion is persisted separately from the Windows onboarding state. The dialog intentionally contains no simulated menu-bar screenshots or detached preview copy.

macOS & Windows 首次设置对话框可分别配置经典文字额度栏和紧凑额度图标。两者都能独立启用或关闭，也允许同时关闭；全部视觉选择都会实时更新，完成状态与 Windows 引导分别持久化。界面不再包含模拟菜单栏截图或独立的预览说明文字。

The Windows onboarding dialog configures taskbar placement and the system-tray quota icon before the main application opens. It keeps at least one Windows quota surface active and explains the Windows Widgets placement interaction.

Windows 首次设置对话框会在进入主界面前配置任务栏组件位置与系统托盘额度图标，确保至少启用一种 Windows 额度展示，并说明与 Windows Widgets 位置的关系。

**macOS first-launch quota setup effect / macOS 首次启动额度设置效果**

![macOS first-launch quota setup effect / macOS 首次启动额度设置效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-first-launch-quota-setup.png)

**Windows first-launch quota setup effect / Windows 首次启动额度设置效果**

![Windows first-launch quota setup effect / Windows 首次启动额度设置效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-first-launch-quota-setup.png)

### 4. Runtime recovery and authorization freshness / 运行时恢复与授权新鲜度

Quota surfaces retain explicit fresh, stale, and error states instead of silently clearing a previous refresh failure. After Explorer or taskbar changes, Windows rebinds and lays out the quota surface again, recreating the widget window only if it was actually destroyed. When the active instance receives a newer, rotated authorization snapshot whose refresh timestamp is not older than the stored snapshot, it can clear the obsolete authorization block and resume quota refresh; the same stale snapshot is never treated as recovery.

额度展示会明确保留正常、缓存过期和错误状态，不再因普通设置刷新而静默清除此前的额度错误。Explorer 或任务栏变化后，Windows 会重新绑定并布局额度展示，只有组件窗口确实被销毁时才重新创建。当当前实例获得刷新令牌已轮换、且刷新时间不早于已保存快照的新授权时，可以清除旧授权阻塞并恢复额度刷新；相同的旧快照不会被误判为已经恢复。

On macOS, one native status-bar scheduler refreshes status data every 60 seconds even when both visible quota surfaces are disabled, then emits the fresh result to the frontend. Freshness-only account-store writes are deferred for up to five minutes; material quota, auth, error, startup, manual, and import changes still persist immediately.

在 macOS 上，即使两个可见额度入口都关闭，单一原生状态栏调度器仍每 60 秒刷新一次状态，并将最新结果发送给前端。仅更新时间的新鲜度写盘最多延后五分钟；额度、授权、错误、启动、手动和导入等实质变化仍会立即持久化。

Fresh isolated previews no longer copy the production account store or production `auth.json`. Existing preview data is preserved, so developers can migrate or reauthorize intentionally without destructive startup behavior.

新的隔离预览不再复制正式版账号库或正式版 `auth.json`。已有预览数据保持不变，开发者可以主动迁移或重新授权，不会在启动时发生破坏性覆盖。

## Validation / 验证

### macOS Apple Silicon

- [x] Frontend production build and TypeScript / 前端生产构建与 TypeScript：`npm run build`
- [x] ESLint completed with zero errors and four baseline unused-disable warnings / ESLint 完成，0 个错误、4 个基线 unused-disable 警告：`npm run lint`
- [x] Onboarding tests / 首次设置测试：8 passed
- [x] Usage-error tests / 额度错误测试：8 passed
- [x] Full Rust suite / 完整 Rust 测试：238 passed
- [x] Rust formatting / Rust 格式：`cargo fmt --check`
- [x] Clippy completed successfully; macOS reports cross-platform dead-code warnings for Windows-only rendering helpers / Clippy 成功完成；macOS 对 Windows 专用渲染辅助代码报告跨平台 dead-code 警告
- [x] Debug app and DMG generated; the app launched with isolated data / 已生成 debug 应用与 DMG，并使用隔离数据启动应用
- [x] Computer Use regression: live style updates, both quota surfaces disabled, completion persistence, and restart behavior / Computer Use 回归：视觉样式实时更新、两类额度展示同时关闭、完成状态持久化及重启行为
- [x] Runtime scheduling regression: one 60-second native refresh, frontend event delivery, and five-minute freshness-only persistence throttling / 运行时调度回归：单一 60 秒原生刷新、前端事件传递及五分钟新鲜度写盘节流
- [x] Production dependency audit / 生产依赖审计：0 vulnerabilities

### Windows 11

Windows frontend validation was replayed from the pushed synchronized branch on the ROG-Strix host. Native packaging was not part of this final PR-draft replay. / Windows 前端验证已从推送后的同步分支在 ROG-Strix 主机上重跑；本次 PR 草稿最终回归不包含原生安装包重建。

- [x] Frontend dependency preflight, production build, TypeScript, and ESLint / 前端依赖预检、生产构建、TypeScript 与 ESLint
- [x] Node regression tests / Node 回归测试：19 passed
- [x] Re-enable and placement regressions / 重新启用与位置选择回归：9/9 frontend and 17/17 Windows native tests passed / 前端 9/9、Windows 原生 17/17 通过
- [x] ROG-Strix real-taskbar regression: off → on restores the visible widget; left → right → left remains visible / ROG-Strix 真实任务栏回归：关闭 → 开启恢复可见组件；左侧 → 右侧 → 左侧后仍正常显示
- [x] Windows Rust compilation / Windows Rust 编译：`cargo check` passed / 通过
- [x] Rust formatting / Rust 格式：`cargo fmt --check` passed / 通过
- [x] Production dependency audit / 生产依赖审计：0 vulnerabilities
- [x] Git diff whitespace check / Git 差异空白检查：passed / 通过
