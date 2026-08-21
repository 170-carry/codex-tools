<!-- PR title / PR 标题: [EN/ZH] Add cross-platform quota displays and first-launch setup / 添加跨平台额度展示与首次设置 -->

## Summary / 摘要

- **Add shared quota icon styles / 新增共享额度图标样式:** provide five compact styles for the macOS menu bar and Windows system tray, with live settings updates.
- **Add an experimental native Windows taskbar quota component / 新增实验性 Windows 原生任务栏额度组件:** show quota on the left or right side of the primary taskbar, or keep only the system-tray icon.
- **Add platform-specific first-launch setup / 新增分平台首次启动设置:** configure macOS text/icon quota surfaces and Windows taskbar/tray surfaces before entering the main application.
- **Harden lifecycle, refresh, and validation / 加固生命周期、刷新与验收流程:** stabilize macOS status items, recover Windows taskbar surfaces, preserve meaningful refresh states, and keep preview data isolated.

## Changes / 改动

### 1. Shared quota icon styles and live settings / 共享额度图标样式与实时设置

macOS and Windows now share five compact quota styles: square number card, wide number card, gradient number, number progress bar, and app-icon progress ring. On macOS, the progress-ring choice contains ring-only and ring-with-percentage variants. Users can also hide the compact quota icon, and settings apply immediately.

macOS 和 Windows 现在共享五种紧凑额度样式：方形数字卡、横向数字卡、渐变数字、数字进度条和图标进度环。macOS 的图标进度环包含“仅显示进度环”和“显示百分比”两个子方案。用户也可以隐藏紧凑额度图标，设置会立即生效。

#### macOS quota icon choices / macOS 额度图标选择

![macOS quota icon choices / macOS 额度图标选择](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-quota-icon-style-settings.png)

#### Windows quota icon choices / Windows 额度图标选择

![Windows quota icon choices / Windows 额度图标选择](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-quota-icon-style-settings.png)

#### Actual macOS menu-bar result / macOS 菜单栏实机效果

![Actual macOS menu-bar quota result / macOS 菜单栏额度实机效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-menu-bar-quota-effect.png)

#### Actual Windows tray result / Windows 托盘实机效果

![Actual Windows tray quota result / Windows 托盘额度实机效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-tray-quota-icon-effect.png)

### 2. Experimental native Windows taskbar quota component / 实验性 Windows 原生任务栏额度组件

Windows can experimentally show quota at the far left or right side of the primary taskbar, or keep only the system-tray icon. The native implementation handles Windows Widgets placement, taskbar auto-hide, fullscreen windows, DPI changes, Explorer recreation, premultiplied transparency, and rate-limited UI Automation scans.

Windows 可以通过实验性功能在系统主任务栏最左侧或右侧显示额度，也可以仅保留系统托盘图标。原生实现覆盖 Windows Widgets 位置、任务栏自动隐藏、全屏窗口、DPI 变化、Explorer 重建、预乘透明渲染和限频 UI Automation 扫描。

The component is deliberately pinned to the primary taskbar. Moving the Codex Tools window to a secondary monitor does not move or duplicate the component, avoiding incompatible `Shell_SecondaryTrayWnd` child hierarchies. Because the surface still integrates with the Explorer taskbar window hierarchy, both onboarding and Settings label it as experimental; the independent system-tray icon remains available as a fallback.

该组件会固定在系统主任务栏。将 Codex Tools 窗口移到副屏时，组件不会跟随或复制，从而避开不兼容的 `Shell_SecondaryTrayWnd` 子窗口层级。由于该展示面仍需接入 Explorer 任务栏窗口层级，首次启动页和设置页都会将其标记为实验性功能；独立的系统托盘图标仍可作为回退方案。

Turning the component off keeps its transparent child window attached to the taskbar. Turning it back on refreshes the native surface so its pixels return reliably. Selecting either taskbar placement during first-launch setup also re-enables a previously hidden component.

关闭组件时会保留其挂载在任务栏上的透明子窗口；重新开启时会刷新原生表面，使额度像素可靠恢复。在首次启动设置中选择任一任务栏位置，也会重新启用此前隐藏的组件。

Windows Settings now exposes the four usable quota modes: remaining, used, five-hour remaining, and one-week remaining. A legacy or imported `hidden` text mode is treated as the default one-week mode on Windows, so selecting a taskbar position cannot leave the component invisibly disabled.

Windows 设置现在提供四种可用额度口径：剩余、已用、仅 5 小时剩余和仅 1 周剩余。历史或导入数据中的 `hidden` 文字模式在 Windows 上会安全回退为默认的“仅 1 周剩余”，避免已经选择任务栏位置但组件仍被隐式隐藏。

#### Actual Windows taskbar result / Windows 任务栏实机效果

![Actual Windows taskbar quota result / Windows 任务栏额度实机效果](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-taskbar-quota-effect.png)

### 3. Platform-specific first-launch setup / 分平台首次启动设置

The macOS dialog configures two independent surfaces. The text quota display can use either the Codex Tools icon or a progress-ring icon, while the compact quota icon can use any of the five shared styles. Either surface can be enabled independently, both can remain enabled, and both can also remain disabled.

macOS 对话框分别配置两个互相独立的展示入口。文字额度栏可以使用 Codex Tools 图标或进度环图标，紧凑额度图标可以使用五种共享样式中的任意一种。两个入口可以分别启用，也可以同时启用或同时关闭。

The classic text choice uses a tightly cropped real menu-bar sample. A new user without an account sees a temporary 100% menu-bar preview while onboarding is incomplete, so the selected result is visible before an account is added.

经典文字方案使用经过紧凑裁切的真实菜单栏示例。未添加账号的新用户在完成引导前会看到临时的 100% 菜单栏预览，因此可以在添加账号前看到所选方案的实际效果。

The Windows dialog configures taskbar placement and the system-tray quota icon before the main application opens. Windows keeps at least one quota surface enabled and explains how Windows Widgets affects left-side placement.

Windows 对话框会在进入主界面前配置任务栏位置和系统托盘额度图标。Windows 会保持至少一种额度展示处于启用状态，并说明 Windows Widgets 对任务栏左侧位置的影响。

#### macOS first-launch setup / macOS 首次启动设置

![macOS first-launch quota setup / macOS 首次启动额度设置](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/macos-first-launch-quota-setup.png)

#### Windows first-launch setup / Windows 首次启动设置

![Windows first-launch quota setup / Windows 首次启动额度设置](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/macos-first-launch-quota/docs/pr-assets/quota-status-visuals/windows-first-launch-quota-setup.png)

### 4. Status-item and refresh reliability / 状态项与刷新可靠性

The macOS text and compact quota surfaces have separate persistent status-item identities. Old native items are removed before replacements are created, preventing an older wrapper from unregistering a newly created item. Three-digit quota values, including 100, are constrained to the icon canvas so they are not clipped.

macOS 的文字额度栏和紧凑额度图标拥有各自独立、持久的状态项身份。创建替代项前会先移除旧原生状态项，防止旧包装对象反向注销刚创建的新状态项。包括 100 在内的三位数额度会被限制在图标画布内，避免显示不完整。

Quota surfaces keep explicit fresh, stale, and error states instead of clearing a previous refresh failure during an unrelated settings update. After Explorer or taskbar changes, Windows rebinds and lays out the existing quota surface, recreating its window only when that window was actually destroyed.

额度展示会明确保留正常、缓存过期和错误状态，不会在无关的设置更新中清除此前的刷新错误。Explorer 或任务栏变化后，Windows 会重新绑定并布局现有额度展示，只有组件窗口确实被销毁时才重新创建窗口。

When a genuinely newer rotated authorization snapshot arrives, the app can clear an obsolete authorization block and resume quota refresh. Reusing the same stale snapshot cannot falsely unlock refresh.

当应用收到确实更新、且刷新令牌已经轮换的授权快照时，可以清除过时的授权阻塞并恢复额度刷新；重复使用同一份旧快照不会被误判为已经恢复。

### 5. Background work and isolated macOS validation / 后台工作与 macOS 隔离验收

macOS uses one native 60-second status refresh scheduler and sends fresh results to the frontend. Writes that only update freshness timestamps are deferred for up to five minutes, while quota, authorization, error, startup, manual, and import changes are still saved immediately.

macOS 使用一个原生 60 秒状态刷新调度器，并将最新结果发送给前端。仅更新时间戳的新鲜度写盘最多延后五分钟；额度、授权、错误、启动、手动和导入等实质变化仍会立即保存。

Fresh preview environments no longer copy the production account store or production `auth.json`. Debug macOS builds expose the real first-launch dialog from both Settings and the application menu, without a global keyboard shortcut. The validation script uses stable bundle identities, refuses concurrent `com.carry.codex-tools*` variants, and performs a best-effort read-only check for confirmed Control Center status-item pollution.

新的预览环境不再复制正式版账号库或正式版 `auth.json`。macOS 调试构建可以从设置页和应用菜单重新打开真实首次启动对话框，并且不占用全局快捷键。验收脚本使用固定的应用身份，拒绝同时运行多个 `com.carry.codex-tools*` 变体，并以只读方式尽力检查已经确认的 Control Center 状态项污染。

## Validation / 验证

### macOS Apple Silicon

- [x] Frontend production build and TypeScript / 前端生产构建与 TypeScript：`npm run build`
- [x] ESLint completed with zero errors and four pre-existing unused-disable warnings / ESLint 完成，0 个错误、4 个既有 unused-disable 警告：`npm run lint`
- [x] First-launch setup tests / 首次启动设置测试：10 passed
- [x] Usage-error presentation tests / 额度错误展示测试：8 passed
- [x] Full Rust suite / 完整 Rust 测试：246 passed
- [x] Rust formatting / Rust 格式：`cargo fmt --manifest-path src-tauri/Cargo.toml --check`
- [x] Git whitespace validation / Git 空白差异检查：`git diff --check`
- [x] Release `.app` packaging with production Bundle ID, ad-hoc/no Team ID signing, and no debug-only quota-preview UI / Release `.app` 已使用正式 Bundle ID 完成打包，仅为 ad-hoc、无 Team ID，且不包含额度预览调试入口
- [x] Debug acceptance app built and launched with isolated runtime and application data / Debug 验收应用已使用隔离的运行目录和应用数据完成构建、启动
- [x] Live UI regression: Settings preview entry, application-menu entry without a shortcut, and real first-launch dialog reopening / 实时界面回归：设置页预览入口、无快捷键的应用菜单入口和真实首屏重新打开
- [x] Menu-bar regression: independent text/icon visibility, live style changes, completion persistence, restart behavior, and new-user 100% preview / 菜单栏回归：文字与图标独立显示、样式实时切换、完成状态持久化、重启行为和新用户 100% 预览
- [x] Runtime regression: one native refresh scheduler, frontend event delivery, and five-minute freshness-only write throttling / 运行时回归：单一原生刷新调度器、前端事件传递和五分钟新鲜度写盘节流
- [x] npm production dependency audit / npm 生产依赖审计：0 vulnerabilities

### Windows 11

- [x] Exact release identity / 精确发布身份：HEAD `6e96f6edcf25b96067a6428a35e6e5c2c0cfdfc2`, tree `35ca2b26de20263859b328481a248ee88f9aede4`
- [x] Frontend dependency preflight, production build, TypeScript, and ESLint / 前端依赖预检、生产构建、TypeScript 与 ESLint
- [x] Node regression tests / Node 回归测试：18 passed
- [x] Re-enable and placement regressions / 重新启用与位置回归：10/10 frontend and 18/18 Windows native tests passed / 前端 10/10、Windows 原生测试 18/18 通过
- [x] Real taskbar regression: left and right both survive visible → hidden → visible transitions / 真实任务栏回归：主任务栏左侧和右侧均通过显示 → 隐藏 → 恢复测试
- [x] Final settings migration regression: legacy `hidden` mode migration tests passed 3/3; hidden state survived Explorer recreation and re-enabling restored the component / 设置迁移最终回归：历史 `hidden` 模式迁移测试 3/3 通过；隐藏状态在 Explorer 重建后保持，重新启用后组件恢复
- [x] Exact-candidate Rust compilation / 精确候选 Rust 编译：`cargo check --all-targets --all-features --offline`
- [x] Exact-candidate Windows release build / 精确候选 Windows Release 构建：passed / 通过
- [x] Mixed-DPI dual-monitor gate: 2560×1600 at 150% primary display plus 3072×1920 at 200% secondary display / 混合 DPI 双屏门禁：主屏 2560×1600、150%，副屏 3072×1920、200%
- [x] Primary-taskbar pinning: moving the main window to the secondary display kept the component on `Shell_TrayWnd`, with no secondary copy or residual pixels / 主任务栏固定：主窗口移到副屏后，组件仍挂载在 `Shell_TrayWnd`，副屏没有副本或残留像素
- [x] Native taskbar pixel gate A: primary-taskbar left and right placements remained visible while the main window was on either display / 原生任务栏像素门禁 A：无论主窗口位于主屏还是副屏，主任务栏左侧和右侧均保持可见
- [x] Native taskbar pixel gate B: visible left placement returned after Explorer restart on the recreated primary taskbar / 原生任务栏像素门禁 B：Explorer 重启后，左侧组件在重建的主任务栏中恢复
- [x] Native taskbar pixel gate C: hidden state survived Explorer restart and re-enabling left placement restored real pixels / 原生任务栏像素门禁 C：隐藏状态在 Explorer 重启后保持，重新启用左侧后真实像素恢复
- [x] Native taskbar pixel gate D: right placement returned after Explorer restart without creating a secondary-taskbar copy / 原生任务栏像素门禁 D：Explorer 重启后右侧组件恢复，且未在副任务栏创建副本
- [x] Rust formatting / Rust 格式：`cargo fmt --check`
- [x] Dependency installation / 依赖安装：`npm ci` completed; npm reported 9 advisories in the complete dependency tree / `npm ci` 完成；npm 对完整依赖树报告 9 项 advisory
- [x] npm production dependency audit / npm 生产依赖审计：0 vulnerabilities
- [x] Git whitespace validation / Git 空白差异检查：passed

Related issue / 关联问题：#168
