<!-- PR title / PR 标题: [EN/ZH] Add cross-platform quota displays and first-launch setup / 添加跨平台额度展示与首次设置 -->

## Summary / 摘要

- **Add shared quota icon styles / 新增共享额度图标样式:** provide five compact styles for the macOS menu bar and Windows system tray, with live settings updates. / 为 macOS 菜单栏和 Windows 系统托盘提供五种紧凑样式，并支持设置实时生效。
- **Add an experimental native Windows taskbar quota component / 新增实验性 Windows 原生任务栏额度组件:** show quota on the left or right side of the primary taskbar, or keep only the system-tray icon. / 在主任务栏左侧或右侧显示额度，也可以仅保留系统托盘图标。
- **Add platform-specific first-launch setup / 新增分平台首次启动设置:** configure macOS text/icon quota surfaces and Windows taskbar/tray surfaces before entering the main application. / 在进入主界面前分别配置 macOS 文字与图标额度展示，以及 Windows 任务栏与托盘展示。
- **Harden refresh and validation / 加固刷新与验收流程:** recover renewed authorization snapshots, coordinate 60-second quota refreshes across platforms, restore Windows taskbar surfaces after Explorer restarts, and keep preview data isolated. / 恢复已经更新的授权快照，统一跨平台 60 秒额度刷新，在 Explorer 重启后恢复 Windows 任务栏组件，并隔离预览数据。

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

### 2. Experimental native Windows taskbar quota component / Windows 原生任务栏额度组件（实验性功能）

Windows can experimentally show quota at the far left or right side of the primary taskbar, or keep only the system-tray icon. The native implementation handles Windows Widgets placement, taskbar auto-hide, fullscreen windows, DPI changes, Explorer recreation, premultiplied transparency, and rate-limited UI Automation scans.

Windows 可以通过该实验性功能在系统主任务栏最左侧或右侧显示额度，也可以仅保留系统托盘图标。原生实现覆盖 Windows Widgets 位置、任务栏自动隐藏、全屏窗口、DPI 变化、Explorer 重建、预乘透明渲染和限频 UI Automation 扫描。

The component is deliberately pinned to the primary taskbar. Moving the Codex Tools window to a secondary monitor does not move or duplicate the component, avoiding incompatible `Shell_SecondaryTrayWnd` child hierarchies. Because the surface still integrates with the Explorer taskbar window hierarchy, both onboarding and Settings label it as experimental; the independent system-tray icon remains available as a fallback.

该组件会固定在系统主任务栏。将 Codex Tools 窗口移到副屏时，组件不会跟随或复制，从而避开不兼容的 `Shell_SecondaryTrayWnd` 子窗口层级。由于该展示面仍需接入 Explorer 任务栏窗口层级，首次启动页和设置页都会将其标记为实验性功能；独立的系统托盘图标仍可作为回退方案。

After Explorer restarts, a visible component returns to its previous primary-taskbar placement. If the user hides it, it remains hidden through the restart; selecting the left or right placement in Settings or first-launch setup makes it visible again.

Explorer 重启后，正在显示的组件会回到此前的主任务栏位置；如果用户主动将其隐藏，则重启后仍保持隐藏。在设置或首次启动界面重新选择主任务栏左侧或右侧，即可再次显示。

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

### 4. Authorization recovery / 授权恢复

In the previous version, an account could remain blocked even after its login credentials (`auth.json`) had been updated, because the blocked refresh path did not inspect the newer authorization snapshot. The app now recognizes a genuinely newer rotated credential, clears the obsolete block, and prevents account deduplication from restoring it. An unchanged or older snapshot is not misclassified as a successful recovery.

在旧版本中，即使 `auth.json` 已经更新，账号仍可能继续处于授权刷新受阻状态，因为被阻止的刷新链路不会检查新的授权快照。现在会识别确实更新且已经轮换的凭据，清除过时的阻塞状态，并避免账号去重重新恢复该状态；相同或更旧的凭据不会被误判为恢复成功。

### 5. Quota refresh and persistence improvements / 额度刷新与写入机制优化

In the previous version, quota refresh behavior depended on whether the main window was visible and whether a persistent quota surface was enabled, and quota could remain unchanged for up to five minutes. The app now uses a fixed 60-second refresh schedule. The same result updates the menu bar, taskbar or system tray, and the application window.

旧版本的额度刷新机制区分主窗口是否可见、是否启用了常驻额度展示，最长不更新时间为五分钟。现在使用固定 60 秒额度刷新调度。同一份刷新结果会同步更新菜单栏、任务栏或系统托盘，以及应用窗口。

In the previous version, startup, manual, frontend, and native status-bar refreshes could run independently, so overlapping refreshes could repeat the same network work. Every completed refresh also saved the account store, even when only freshness timestamps changed. Now startup, manual, and periodic refreshes share one coordinator, so overlapping requests no longer duplicate network work. For periodic refreshes, timestamp-only changes are written at most once every five minutes, while quota, authorization, and error changes are still saved immediately.

在旧版本中，启动、手动、前端和原生状态栏刷新会分别运行，同时发生时可能重复发起相同的网络请求。每次刷新完成后也都会保存账号库，即使变化只有新鲜度时间戳。现在，启动、手动和周期刷新共用同一个协调器，重叠请求不会重复发起网络访问。对于周期刷新，仅更新时间戳的变化最多每五分钟写盘一次；额度、授权和错误等实质变化仍会立即保存。

### 6. Correct the re-login action label / 更正“重新登录”操作字样

In v2.0.0, the “Re-login” label in the account menu and details view was mistakenly changed to “Test login”. It has now been corrected.

账号菜单和详情视图中的“重新登录”字样在 v2.0.0 被误改为“测试登录”，现已更正。

## Validation / 验证

### Isolated validation environment / 隔离验收环境

Fresh Windows preview environments no longer copy the production account store or `auth.json`; existing isolated preview data is preserved. The new macOS menu-bar validation script uses dedicated data and Codex directories with stable Bundle IDs, refuses concurrent `com.carry.codex-tools*` variants, and warns when known stale Control Center status-item identities are detected.

新的 Windows 预览环境不再复制正式版账号库或 `auth.json`，已有的隔离预览数据会继续保留。新增的 macOS 菜单栏验收脚本使用独立的数据与 Codex 目录及固定 Bundle ID，拒绝多个 `com.carry.codex-tools*` 变体并行运行，并在检测到已知的控制中心旧状态项身份残留时发出警告。

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
- [x] Runtime regression: shared native 60-second scheduler, frontend event delivery, and five-minute freshness-only write throttling / 运行时回归：共享原生 60 秒调度器、前端事件传递和五分钟纯时间戳写盘节流
- [x] npm production dependency audit / npm 生产依赖审计：0 vulnerabilities

### Windows 11

- [x] Exact current-candidate Windows build and isolated runtime check: the native scheduler continued at about 61-second intervals with the main window both visible and hidden, legacy periodic sources stayed inactive, and the taskbar child surface remained visible under `Shell_TrayWnd` / 当前精确候选版本 Windows 构建及隔离实机检查：主窗口可见和隐藏时，原生调度均保持约 61 秒间隔，旧周期刷新来源均未触发，任务栏子组件在 `Shell_TrayWnd` 下保持可见
- [x] Frontend dependency preflight, production build, TypeScript, and ESLint / 前端依赖预检、生产构建、TypeScript 与 ESLint
- [x] Node regression tests / Node 回归测试：18 passed
- [x] Current-candidate Windows full Rust suite / 当前候选版本 Windows 完整 Rust 测试：259 passed
- [x] Re-enable and placement regressions / 重新启用与位置回归：10/10 frontend and 18/18 Windows native tests passed / 前端 10/10、Windows 原生测试 18/18 通过
- [x] Real taskbar regression: left and right both survive visible → hidden → visible transitions / 真实任务栏回归：主任务栏左侧和右侧均通过显示 → 隐藏 → 恢复测试
- [x] Final settings migration regression: legacy `hidden` mode migration tests passed 3/3; hidden state survived Explorer recreation and re-enabling restored the component / 设置迁移最终回归：历史 `hidden` 模式迁移测试 3/3 通过；隐藏状态在 Explorer 重建后保持，重新启用后组件恢复
- [x] Previously tested candidate Rust compilation / 先前候选版本 Rust 编译：`cargo check --all-targets --all-features --offline`
- [x] Previously tested candidate Windows release build / 先前候选版本 Windows Release 构建：passed / 通过
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
