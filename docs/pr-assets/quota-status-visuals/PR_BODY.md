<!-- PR title / PR 标题: [EN/ZH] Add cross-platform quota status visuals and a Windows taskbar widget / 添加跨平台额度状态视觉与 Windows 任务栏组件 -->

> **Draft status / 草稿状态:** The feature branch is based on the latest `origin/main` v2.6.0. macOS validation and the synchronized ROG-Strix Windows frontend replay passed. The Windows first-launch screenshot is the only remaining PR visual. Local updater metadata still requires the repository's `TAURI_SIGNING_PRIVATE_KEY`. / 当前功能分支已基于最新的 `origin/main` v2.6.0。macOS 验证与 ROG-Strix Windows 前端同步回归均已通过。Windows 首次启动截图是 PR 当前唯一待补视觉素材。本地更新元数据仍需仓库的 `TAURI_SIGNING_PRIVATE_KEY` 才能签名。

## Summary / 摘要

- **Add consistent quota visuals across desktop surfaces / 为桌面额度展示提供一致视觉:** share five quota-icon styles across Windows and macOS, including two variants inside the logo-and-progress-ring style.
- **Add a native Windows taskbar quota widget / 新增 Windows 原生任务栏额度组件:** place quota information on the left or right side of the taskbar, or keep it in the system tray, with live settings updates.
- **Add cross-platform first-launch quota setup / 新增跨平台首次启动额度设置:** let macOS users configure the text quota surface and compact quota icon independently, and let Windows users choose taskbar placement and tray presentation before entering the main application.
- **Harden recovery and isolated development / 加固恢复逻辑与隔离开发:** recover taskbar surfaces after Explorer changes, preserve cached error states, and safely adopt newer authorization snapshots without cloning production credentials into a fresh preview environment.
- **Reduce macOS background work / 降低 macOS 后台开销:** use one native status refresh scheduler, deliver fresh results to the UI, and defer freshness-only account-store writes.

## Changes / 改动

### 1. Shared quota visual styles and settings / 共享额度视觉样式与设置

The menu bar and taskbar surfaces now support a square number card, wide number card, gradient number, number progress bar, and app-icon progress ring. The app-icon ring style contains ring-only and ring-with-percentage variants. The same stored configuration drives supported Windows and macOS surfaces, and changes apply immediately.

菜单栏与任务栏展示现在支持方形数字卡、横向数字卡、渐变数字、数字进度条和图标进度环；图标进度环同时提供“仅显示进度环”和“显示百分比”两个子方案。Windows 与 macOS 的受支持展示面共用同一套持久化配置，修改后立即生效。

**macOS menu-bar quota effect / macOS 菜单栏额度效果**

![macOS menu-bar quota effect / macOS 菜单栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-menu-bar-quota-effect.png)

**Windows tray quota icon effect / Windows 托盘额度图标效果**

![Windows tray quota icon effect / Windows 托盘额度图标效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-tray-quota-icon-effect.png)

### 2. Native Windows taskbar and tray quota surfaces / Windows 原生任务栏与托盘额度展示

Windows can render quota information as a native taskbar child surface on either side of the taskbar or as a system-tray icon. The implementation handles Windows Widgets placement, taskbar auto-hide, fullscreen windows, DPI and Explorer recreation, uses premultiplied transparency, and rate-limits UI Automation scans.

Windows 可将额度作为原生任务栏子组件放在任务栏左侧或右侧，也可以仅使用系统托盘图标。实现覆盖 Windows Widgets 位置、任务栏自动隐藏、全屏窗口、DPI 与 Explorer 重建，采用预乘透明渲染，并限制 UI Automation 扫描频率。

**Windows taskbar quota effect / Windows 任务栏额度效果**

![Windows taskbar quota effect / Windows 任务栏额度效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-taskbar-quota-effect.png)

### 3. Cross-platform first-launch quota setup / 跨平台首次启动额度设置

The macOS onboarding dialog configures classic status text and the compact quota icon independently. Either surface can be enabled or disabled, both can remain disabled, all visual choices update live, and completion is persisted separately from the Windows onboarding state. The dialog intentionally contains no simulated menu-bar screenshots or detached preview copy.

macOS 首次设置对话框可分别配置经典文字额度栏和紧凑额度图标。两者都能独立启用或关闭，也允许同时关闭；全部视觉选择都会实时更新，完成状态与 Windows 引导分别持久化。界面不再包含模拟菜单栏截图或独立的预览说明文字。

The Windows onboarding dialog configures taskbar placement and the system-tray quota icon before the main application opens. It keeps at least one Windows quota surface active and explains the Windows Widgets placement interaction.

Windows 首次设置对话框会在进入主界面前配置任务栏组件位置与系统托盘额度图标，确保至少启用一种 Windows 额度展示，并说明与 Windows Widgets 位置的关系。

**macOS first-launch quota setup effect / macOS 首次启动额度设置效果**

![macOS first-launch quota setup effect / macOS 首次启动额度设置效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-first-launch-quota-setup.png)

**Windows first-launch quota setup effect / Windows 首次启动额度设置效果**

- [ ] Add the current Windows first-launch screenshot before opening the PR. / 创建 PR 前补充当前 Windows 首次启动截图。

### 4. Runtime recovery and authorization freshness / 运行时恢复与授权新鲜度

Quota surfaces retain explicit fresh, stale, and error states instead of silently clearing a previous refresh failure. After Explorer or taskbar changes, Windows rebinds and lays out the quota surface again, recreating the widget window only if it was actually destroyed. When the active instance receives a newer, rotated authorization snapshot whose refresh timestamp is not older than the stored snapshot, it can clear the obsolete authorization block and resume quota refresh; the same stale snapshot is never treated as recovery.

额度展示会明确保留正常、缓存过期和错误状态，不再因普通设置刷新而静默清除此前的额度错误。Explorer 或任务栏变化后，Windows 会重新绑定并布局额度展示，只有组件窗口确实被销毁时才重新创建。当当前实例获得刷新令牌已轮换、且刷新时间不早于已保存快照的新授权时，可以清除旧授权阻塞并恢复额度刷新；相同的旧快照不会被误判为已经恢复。

On macOS, one native status-bar scheduler refreshes status data every 60 seconds even when both visible quota surfaces are disabled, then emits the fresh result to the frontend. Freshness-only account-store writes are deferred for up to five minutes; material quota, auth, error, startup, manual, and import changes still persist immediately.

在 macOS 上，即使两个可见额度入口都关闭，单一原生状态栏调度器仍每 60 秒刷新一次状态，并将最新结果发送给前端。仅更新时间的新鲜度写盘最多延后五分钟；额度、授权、错误、启动、手动和导入等实质变化仍会立即持久化。

Fresh isolated previews no longer copy the production account store or production `auth.json`. Existing preview data is preserved, so developers can migrate or reauthorize intentionally without destructive startup behavior.

新的隔离预览不再复制正式版账号库或正式版 `auth.json`。已有预览数据保持不变，开发者可以主动迁移或重新授权，不会在启动时发生破坏性覆盖。

### 5. Documentation and compatibility / 文档与兼容性

- Document Windows taskbar placement, first-run behavior, and the Windows Widgets interaction. / 说明 Windows 任务栏位置、首次运行行为以及与 Windows Widgets 的交互。
- Preserve existing account, API proxy, analytics, editor integration, and non-Windows behavior. / 保持现有账号、API 反代、分析、编辑器联动及非 Windows 行为。

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

### Windows 11 on ROG Strix

Windows frontend validation was replayed from the pushed synchronized branch on the ROG-Strix host. Native packaging was not part of this final PR-draft replay. / Windows 前端验证已从推送后的同步分支在 ROG-Strix 主机上重跑；本次 PR 草稿最终回归不包含原生安装包重建。

- [x] Frontend dependency preflight, production build, TypeScript, and ESLint / 前端依赖预检、生产构建、TypeScript 与 ESLint
- [x] Node regression tests / Node 回归测试：19 passed
- [x] Rust formatting / Rust 格式：`cargo fmt --check` passed / 通过
- [x] Production dependency audit / 生产依赖审计：0 vulnerabilities
- [x] Git diff whitespace check / Git 差异空白检查：passed / 通过

## Known non-blocking baseline / 已知非阻断基线

- The minified frontend entry chunk is approximately 706 kB and still triggers Vite's existing 500 kB recommendation. / 压缩后的前端入口约为 706 kB，仍会触发 Vite 既有的 500 kB 拆包建议。
- The full development-dependency audit reports 9 advisories (2 low and 7 high) on both `origin/main` and this branch; production dependencies report 0. Patch-level lockfile upgrades are available but are outside this feature diff. / `origin/main` 与本分支的完整开发依赖审计均报告 9 个公告（2 个 low、7 个 high）；生产依赖为 0。可通过补丁级锁文件升级处理，但不属于本功能差异。
- Locally built macOS bundles are suitable for validation but are not Developer ID signed or notarized. / 本地构建的 macOS 包可用于验证，但未使用 Developer ID 签名或公证。
- Local updater metadata cannot be signed without `TAURI_SIGNING_PRIVATE_KEY`. / 缺少 `TAURI_SIGNING_PRIVATE_KEY` 时无法在本地签署更新元数据。

## Review notes / 审阅说明

- The screenshots contain no account identifiers or credentials; validation-only captures use isolated application data. / 截图不包含账号标识或凭据；仅用于验证的截图使用隔离应用数据。
- The visual evidence shows only the new quota surfaces and first-launch UI. Because these are newly added features, no before/after comparison is included, and settings-page screenshots remain excluded. The Windows first-launch screenshot is still pending. / 视觉证据仅展示新增的额度栏与首次启动界面。由于这些均为新增功能，因此不提供修改前后对比，也不包含设置页截图。Windows 首次启动截图仍待补充。
- All source changes and selected evidence files are synchronized in the pushed branch; the isolated ROG-Strix validation checkout and the original macOS development worktree remain preserved. / 所有源码修改与选定证据文件已同步到推送分支；隔离的 ROG-Strix 验证检出和原 macOS 开发工作树均保持不变。
