## 更新日志 / Changelog

### Unreleased

- v2.6.0

#### English

1. Add Relay-aware remote compaction and compressed request support: expose the explicit `/v1/responses/compact` route, keep Codex `remote_compaction_v2` on `/v1/responses`, preserve sticky turn context, service tier and prompt cache fields, and accept bounded zstd, gzip and HTTP deflate bodies.
2. Harden the API proxy boundary: authenticate before reading request bodies, limit concurrent decoding, decompressed size and expansion ratio, cap compact upstream responses, preserve actionable upstream errors, and cancel long-running compact work when the client disconnects.
3. Move API proxy usage history to SQLite WAL with a background writer, cross-process-safe legacy migration and schema upgrades, atomic clearing of legacy JSON, SQL bucket aggregation, and non-overlapping polling only while the proxy page is visible.
4. Add CSV export by time range and platform Key, including event details and per-Key/per-model summaries for calls and input, output, cached-input and total tokens when known. Exports use a consistent writer barrier and read snapshot, private atomic files, and spreadsheet-formula protection without exposing secret Key values.
5. Fix the macOS custom title bar so a primary-button drag still moves the window while a double click toggles maximization.
6. Render account action menus through a viewport-positioned portal so scrolling containers no longer clip their options.
7. Hide macOS-only status-item title settings on unsupported platforms instead of presenting controls that silently do nothing.

#### 中文

1. 增加支持 Relay 的远程压缩与压缩请求：开放显式 `/v1/responses/compact` 路由，保持 Codex `remote_compaction_v2` 继续走 `/v1/responses`，保留 turn-state、service tier 与 prompt cache 字段，并在明确上限内支持 zstd、gzip 和标准 HTTP deflate 请求体。
2. 加固 API 反代边界：读取请求体前先完成鉴权，限制解码并发、解压后大小和膨胀比，限制 compact 上游响应体，保留可判断的上游错误；客户端断开时会取消长时间运行的 compact 请求。
3. 将 API 反代用量历史迁移到 SQLite WAL：使用后台 writer，跨进程安全地执行旧数据迁移和 schema 升级，清空时同步删除旧 JSON，通过 SQL 分桶聚合，并仅在反代页面可见时执行不重叠轮询。
4. 增加按时间范围和平台 Key 导出 CSV：包含事件明细及按 Key/模型汇总的调用数，并在可识别时导出 input、output、cached-input 和 total token。导出使用 writer barrier 与一致只读快照、私有原子文件和公式注入防护，不包含秘密 Key 值。
5. 修复 macOS 自定义标题栏：鼠标主键拖动仍可移动窗口，双击则切换最大化。
6. 账号操作菜单改为通过按视口定位的 portal 渲染，不再被滚动容器裁切。
7. 在不支持的平台隐藏仅对 macOS 状态栏标题生效的设置，避免展示静默无效的开关。

- v2.5.0

#### English

- Add Windows quota surfaces: a native taskbar child component with left/right placement, five system-tray quota icon styles, a first-run setup guide with live preview, Windows Widgets detection, fullscreen and auto-hide handling, Explorer restart recovery, and cached error/stale states. The taskbar component now uses premultiplied transparency and rate-limits UI Automation scans.

1. Improve the macOS status bar and usage labels: use the color app icon, default to showing one-week remaining usage, optionally show 5h / 1w labels, hide the entire status item when disabled, and keep the account meters labeled 5h / 1w even when both values are currently identical.
2. Significantly reduce default background energy use: remove Codex inference keepalive calls, deduplicate refreshes, pause foreground polling while the application's entire main window is hidden, and avoid repeated full log scans by reusing unchanged per-file results and tail-reading appended bytes for both the Token summary and detailed cost analytics. Detailed analytics now refreshes incrementally every minute as a fixed behavior; entering Analytics no longer starts a separate refresh, and the refresh-mode toggle has been removed.
3. Improve first-launch account feedback: show stored accounts and the last saved quota snapshot immediately, refresh remote quota and non-critical startup work concurrently, show freshness only during first-load work, and hide the freshness badge after a successful refresh. Failed or unavailable states remain visible; failures show a concise cause while retaining the full error on hover.
4. Fix startup and the macOS status bar after a Plus-to-Pro upgrade: reuse the cached Pro account when stale auth metadata still says Plus, keep the last quota visible during refresh, and correctly select the current account.
5. Unify local Token analytics and the 7-day heatmap: derive actual increments from cumulative Token snapshots, ignore unchanged rebroadcasts, and exclude verified parent-history replays from forked sessions. Hourly buckets use local time, retain useful color contrast, localize labels, and show exact Token counts immediately on hover. Total, seven-day, project, session, prompt, and heatmap costs now share the same local-log model pricing. The seven-day card uses the previous seven completed local calendar days, while the cost alert uses the rolling latest 168 hours. Analytics no longer requests or caches official Profile activity.
6. Preserve shared Codex configuration when switching accounts: MCP servers, hooks, features, pet settings, and other user-managed keys now follow the active configuration, while model-provider fields remain account-specific.
7. Reject session imports without a usable refresh token and direct users to OAuth or a complete `auth.json`, preventing accounts that cannot refresh from being saved.
8. Improve API proxy observability: fix Unix-second timestamps in recent logs and allow usage charts to switch between model and API-key dimensions.
9. Harden sensitive files and credentials: create private files with restrictive permissions, compare proxy keys in constant time, and avoid printing the full proxy key in daemon output.
10. Keep deployed remote proxies synchronized after account changes and discover Zig installed through WinGet on Windows.

#### 中文

- 新增 Windows 额度展示：提供可置于任务栏左侧或右侧的原生子组件、五种系统托盘额度图标，以及带实时预览的首次运行引导；支持检测 Windows 天气小组件、全屏和自动隐藏处理、Explorer 重启恢复，并保留错误与缓存过期状态。任务栏组件现采用预乘透明渲染，并限制 UI Automation 扫描频率。

1. 优化 macOS 状态栏和用量标签：使用彩色应用图标，默认仅显示一周剩余用量，可选显示 5h / 1w 标签，选择“不显示”时隐藏整个状态项；即使两个周期当前数值相同，账号用量栏仍分别标注 5h / 1w。
2. 大幅降低默认模式下的后台耗电：移除 Codex 推理保活请求，去重刷新任务，在应用的整个主窗口隐藏时暂停前台轮询；Token 汇总与详细成本分析都会复用未变化文件的逐文件结果，并对增长日志采用尾量读取，避免反复完整扫描。详细分析现固定为每分钟增量刷新；进入分析页不再额外触发刷新，并移除刷新模式 toggle。
3. 改善首次启动账号反馈：立即显示本地账号与上次保存的额度快照，并发刷新远端额度和非关键启动任务；新鲜度提示仅在首次加载期间出现，刷新成功后自动隐藏。失败或暂无数据状态仍会保留，其中失败提示直接显示简短原因，悬停时仍可查看完整错误。
4. 修复 Plus 升级为 PRO 后的启动与状态栏账号识别：当认证元数据仍显示 Plus 时复用带缓存的 PRO 账号，刷新期间继续显示上次额度，并正确选中当前账号。
5. 统一本机 Token 分析与 7 日热力图：根据累计 Token 快照计算实际增量，忽略累计值未变化的重复广播，并排除 fork 会话中已验证的父会话历史回放。小时数据按本地时间分桶，保留有效色阶，标签跟随界面语言，并在悬停时立即显示精确 Token 数量。总成本、7 日成本、项目、会话、prompt 与热力图现共用相同的本机日志模型计价。7 日成本采用前 7 个完整本地自然日，成本预警采用滚动最近 168 小时。分析页不再请求或缓存官方 Profile 活动量。
6. 切换账号时保留共享 Codex 配置：MCP、hooks、features、宠物设置及其他用户配置跟随当前配置保存，模型供应商字段仍按账号隔离。
7. 拒绝导入缺少有效 refresh token 的会话，并引导使用 OAuth 或完整 `auth.json`，避免保存后无法刷新的账号。
8. 改善 API 反代可观测性：修复近期日志 Unix 秒级时间戳显示错误，并支持用量图表按模型或 API Key 两种维度查看。
9. 加固敏感文件与凭证：敏感文件创建即使用严格权限，代理密钥采用常量时间比较，守护进程不再输出完整代理密钥。
10. 账号变化后自动同步已部署的远程代理，并在 Windows 上发现通过 WinGet 安装的 Zig。

- v2.4.0
  1. macOS 支持启动新版 `ChatGPT.app`，并继续兼容旧版 `Codex.app`、`Codex Desktop.app` 与 `codex app` 回退
  2. 切换账号时按已验证的桌面主进程树安全重启 ChatGPT/Codex，避免误杀终端独立运行的 Codex CLI
  3. 补充 ChatGPT/Codex 启动兼容测试与使用说明

- v2.3.0
  1. API 反代新增 GPT-5.6 Sol、Terra、Luna，并兼容 `gpt-5.6` 与常见历史别名
  2. 支持完整推理强度与速度入口，默认使用 `gpt-5.6-sol`、`xhigh`、`fast`
  3. GPT-5.6 请求适配 Codex 0.144.0 Responses Lite 契约
  4. 一键绑定本机反代时为缺失配置写入新的模型、推理强度与速度默认值
  5. 补齐远程 proxyd 打包源码，并更新 GPT-5.6 三种模型的成本估算费率

- v2.0.1
  1. 修复 Codex 反代备份目录权限，避免恢复原配置时因目录不可访问失败
  2. 优化用量刷新令牌过期提示，避免误判为账号授权彻底失效

- v2.0.0
  1. 优化了整体UI
  2. 增加CLI
  3. 增加运行热切换
  4. 增加Anthropic Message 兼容
  5. 增加TUI,CLI
  6. API 反代面板增加一键切换 Codex App/CLI 到本机反代地址，并支持恢复原配置
  7. 恢复账号列表全部导出按钮
  8. 桌面端 API 反代改为监听内网地址，内网 Base URL 可被其他设备访问
  9. API 反代模型范围收窄为 gpt-5.4、gpt-5.5、gpt-image-2

- v1.9.5
  1. 修复重新登录问题
  2. API 反代逐个模式增加 session affinity，支持 wrapper/app bind 场景下不关闭 Codex App/CLI 的运行中轮换
- v1.8.9
  1. 同步版本号并发布补丁更新
- v1.8.8
  1. 增加保活
- v1.8.7
  1. 增加账号是否参与 API 反代的开关
  2. 关闭参与反代后，该账号不再进入反代负载均衡候选池
  3. 同步版本号并发布补丁更新
- v1.8.5
  1. 支持通过代理暴露 Codex 账号图片生成能力
  2. 保持最新发布基线下的 Codex 流式兼容性
  3. 同步版本号并发布补丁更新
- v1.8.4
  1. 增加 Codex token 用量统计与展示
  2. 修复 gpt-5.5 代理
  3. 修复若干问题
  4. 同步版本号并发布补丁更新
- v1.8.3
  1. 修复刷新用量后账号 profile 配置不完整提示
  2. 同步版本号并发布补丁更新
- v1.8.2
  1. 同步版本号并发布补丁更新
- v1.8.1
  1. 增强 token 保活刷新，过期 refresh token 会提示重新授权
  2. 支持 gpt-5.5 API 反代模型名映射
- v1.8.0
  1. 同步版本号并发布
- v1.7.7
  1. 修复 Cursor / Codex 插件接入时 `prompt_cache_retention` 参数导致的上游报错
  2. 修复 Windows 下已设置 Codex 启动路径仍无法拉起的问题
  3. 增加微软应用商店安装目录与 WindowsApps 路径探测
  4. 补充 CC Switch 通过 `responses` 协议接入 Codex 反代的文档
- v1.7.6
  1. 兼容官方 Codex 插件使用 `gpt-5.4` 模型名
  2. 保持兼容 `gpt-5-4` 历史别名
- v1.7.5
  1. 修复图标问题
  2. 修复导入导出问题
- v1.7.2
  1. 修复 Windows 11 下重复启动会产生多个实例和托盘图标的问题
- v1.7.1
  1. 修复在windows系统上的稳定性
  2. 修复打包版本号未同步的问题
- v1.7.0
  1. 修复在windows系统上的稳定性
- v1.6.4
  1. 修复代理问题,增加稳定性
- v1.6.3
  1. 修复 Windows 远程连接/部署时临时 SSH 私钥权限过宽导致连接失败的问题
- v1.6.2
  1. 恢复全部导出按钮
- v1.6.1
  1. 增加稳定性,导出支持单选
- v1.6.0
  1. 增加稳定性
- v1.5.5
  1. 修复统一账号切换问题
- v1.5.4
  1. 修复端口占用
- v1.5.3
  1. 修复稳定性
- v1.5.0
  1. 优化整体登录的逻辑和ui
- v1.4.1
  1. 去除工作区相关,因为无法获取到用户工作区
- v1.4.0
  1. 支持自己选择codex路径
  2. 优化opencode 授权
- v1.3.2
  1. 优化UI
- v1.3.1
  1. 增加工作区名字显示
- v1.3.0
  1. 修复同一工作区下不同账号会互相顶掉的问题
  2. 修复账号存储损坏时被重建清空的问题
  3. 补齐远程部署缺少 Rust 工具链时的自动安装路径
- v1.2.1
  1. 增加账号别名编辑
- v1.2.0
  1. 优化设置页信息布局，增加版本信息、GitHub 仓库直达链接、问题反馈入口、版本发布与更新日志入口。
  2. 重做应用图标
- v1.1.3
  1. 修复 Windows 构建失败问题，整理跨平台条件导入
- v1.1.2
  1. 将本地 API 代理上游总超时调整为 30 分钟，避免长请求在约 180 秒时被中断
- v1.1.1
  1. 同步版本号并发布
- v1.1.0
  1. 增加opencode客户端启动
  2. 增加导出功能
  3. 修复若干问题
- v1.0.1
  1. 优化黑色主题,修改多账号同步问题
- v1.0.0
  1. 整体布局重构，更接近原生体验
  2. 优化若干体验问题
  3. 优化远程代理服务器连接
- v0.6.3
  1. 修复反代服务器路径检测问题(打包版走的是 GUI 进程环境，和终端里的 shell 环境不同)
- v0.6.2
  1. 更新刷新错误文案
- v0.6.1
  1. 修复cursor无法代理的问题
- v0.6.0
  1. 增加反代理远程服务器支持 （测试版本，预计会有一些问题）
  2. 支持cloudflared公网访问支持
- v0.5.5
  1. 修复反代理间歇性失效问题
- v0.5.1
  1. 优化布局
- v0.5.0
  1. 增加 API 反代功能
- v0.4.1
  1. 增加日语韩语支持
- v0.4.0
  1. 国际化支持en
  2. 欢迎大家提交pr, 进行国际化开发
- v0.3.3
  1. 增加刷新错误提示
- v0.3.2
  1. 增加用量排序
- v0.3.1
  1. 优化UI表现
- v0.3.0
  1. 修复关闭无效的问题
  2. 优化弹窗样式
- v0.2.7
  1. 增加 用量接口候选
  2. 修复 添加账号偶尔出现消失问题
- v0.2.5
  1. 优化 app 启动退出。
- v0.2.4
  1. 优化整体 UI 布局。
  2. 增加后台运行。
- v0.2.3
  1. 设置增加切换账号是否重启编辑器（兼容 Codex 编辑器插件）。
  2. 修复启动稳定性问题（配置文件报错时自动恢复，避免崩溃）。
  3. 修复 Windows 下 opencode 认证文件目录识别不正确的问题。
- v0.2.2
  1. 设置中增加 Opencode 快捷切换功能，开启后切换 Codex 同时会切换 Opencode 授权。
  2. 优化下载页样式。
- v0.2.1：优化整体启动方式。
