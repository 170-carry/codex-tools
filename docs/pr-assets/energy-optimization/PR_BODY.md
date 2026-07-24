<!-- PR title / PR 标题: [EN/ZH] Improve the macOS status bar and reduce background energy use / 优化 macOS 状态栏并降低后台耗电 -->

> **Draft status / 草稿状态:** The latest account-membership, account-freshness, and analytics refinements are verified locally but not yet pushed to this PR; the four comparison images are available on the PR branch. / 最新的会员信息、账号新鲜度与分析页改动已在本地验证，但尚未推送到本 PR；四张对比图已发布到 PR 分支。

## Summary / 摘要

- **Significantly reduce background energy and system resource use / 大幅降低后台耗电和系统资源占用:** remove inference keepalive work and repeated full log scans, reuse unchanged files, tail-read appended log bytes, and refresh detailed analytics incrementally every minute without a separate page-entry refresh.
- **Improve the macOS status item / 优化 macOS 状态栏:** use the color app icon, add clearer display modes and optional `5h / 1w` labels, and fully hide the status item when disabled.
- **Show useful account data sooner / 更早显示可用账号数据:** render cached accounts first, refresh remote quota in the background, show freshness only during first-load work, and keep concise failure or unavailable states when attention is needed.
- **Correct account identity, authorization, and analytics / 修复账号识别、授权与分析:** resolve Plus-to-Pro accounts by stable identity, preserve refreshed credentials for the active account, show membership expiry as reference-only data, and use one consistent local-log Token and cost algorithm.

## Changes / 改动

### 1. macOS status item and usage labels / macOS 状态栏与用量标签

The status item now uses the color application icon, defaults to one-week remaining usage, can optionally show `5h / 1w` labels through Settings, and disappears completely when “Hidden” is selected. Account meters retain the semantic `5h` and `1w` labels even when both periods currently report the same value.

现在状态栏使用彩色应用图标，默认显示一周剩余用量，可以在设置中选择显示 `5h / 1w` 标签，并在选择“不显示”时完整隐藏。即使两个周期当前数值相同，账号用量栏仍分别标注 `5h` 与 `1w`。

#### Appearance / 外观

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Status item before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-before.png) | ![Status item after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-after.png) |

#### Settings / 设置

| Before / 修改前 |
| --- |
| ![Status settings before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-before.png) |

| After / 修改后 |
| --- |
| ![Status settings after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-after.png) |

#### Usage-window labels / 用量周期标签

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Usage labels before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-before.png) | ![Usage labels after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-after.png) |

### 2. Background energy use / 后台耗电

In the controlled test below, recurring CPU work, attributed reads, and Apple Energy Impact all fell substantially: Coalition CPU by about 97%, attributed reads by nearly 100%, and Energy Impact by about 99%. The app no longer sends an inference keepalive or repeatedly reparses every local Token log. Both the Token summary and detailed cost analytics cache per-file results: unchanged files are reused, while safely append-only logs are read from their previous byte offset (tail-only reads). A truncated, rewritten, or mismatched file falls back to reparsing that file for correctness. Nonessential foreground polling also pauses while the main window is hidden.

在下述受控测试中，应用的周期 CPU 工作、归因读取和 Apple Energy Impact 均显著下降：Coalition CPU 约下降 97%，归因读取接近 100%，Energy Impact 约下降 99%。应用不再发送推理保活请求，也不再反复解析全部本地 Token 日志。Token 汇总与详细成本分析都会缓存逐文件结果：未变化文件直接复用，检测到仅追加的增长日志则从上次字节位置开始读取，即“尾量读取”；若文件被截断、重写或尾部校验不匹配，则为保证正确性回退为重新完整解析该文件。主窗口隐藏时也会暂停非必要前台轮询。

Detailed analytics uses this incremental path automatically every 60 seconds. Opening the Analytics page does not trigger an additional refresh, while **Refresh analytics** remains available for an immediate manual update.

详细分析固定每 60 秒自动执行一次上述增量刷新；进入分析页不会额外触发刷新，同时保留“刷新分析”用于立即手动更新。

![Background work before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/background-work-before-after.png)

Test setup: the modified build and the clean v2.4.0 `main` build were measured on the same Mac in A-B-B-A order, using isolated app data and the same credential-free 229-file log corpus. Each valid round ran for 10 minutes with one Token event appended per minute; the modified build consumed all 9 appended events in both rounds. Values are ordered `main → modified`:

测试设置：修改版与干净的 v2.4.0 `main` 构建在同一台 Mac 上按 A-B-B-A 顺序测试，使用隔离的应用数据及同一份不含凭据的 229 文件日志集。每个有效轮次持续 10 分钟，每分钟追加一条 Token 事件；修改版两轮均读取了全部 9 条新增事件。表中数值顺序为 `main → 修改版`：

| Scenario<br>场景 | Coalition CPU<br>ms/min | Attributed reads<br>归因读取字节<br>B/s | Apple Energy Impact/s |
| --- | ---: | ---: | ---: |
| Default 60-second refresh with a growing log<br>默认每分钟刷新（日志增长） | 12,692 → 336 | 291,028,172 → 30 | 664.13 → 5.02 |

These are macOS process-group attribution metrics, not whole-machine watt-hours. Coalition wakeups increased from 269 to 1,026 per minute in this short run, so this PR claims lower recurring CPU work, attributed reads, and Energy Impact—not fewer wakeups.

这些指标是 macOS 对应用进程组的归因统计，不是整机瓦时。本次短时测试中的 Coalition 唤醒由每分钟 269 次增至 1,026 次，因此本 PR 只主张周期 CPU 工作、归因读取与 Energy Impact 降低，不主张唤醒次数减少。

### 3. Local Token analytics and heatmap / 本地 Token 分析与热力图

Local Token totals now follow one explicit rule: sum every rollout event's `last_token_usage`, without adding cumulative `total_token_usage` snapshots, then remove exact parent-history prefixes copied into forked sessions. The account summary, detailed analytics, and heatmap use the same rule. These are raw local-log Tokens, separate from official quota/Profile metrics and the API proxy's upstream `usage.total_tokens`. Heatmap buckets now use local time and nine logarithmic color levels relative to the largest one-hour bucket in the current seven-day view. This preserves visible differences across large ranges without treating the peak as an account limit. Zero values keep a quiet base color, labels follow the UI language, tooltips appear immediately, and large Token values use compact K/M units.

本地 Token 统一采用一条明确规则：累加日志中每个事件的 `last_token_usage`，不再叠加累计快照 `total_token_usage`，随后排除 fork 会话中与父会话完全一致的复制前缀。账号页汇总、详细分析与热力图共用该规则。这是本地日志的原始 Token，与官方额度/Profile 指标及 API 反代上游返回的 `usage.total_tokens` 相互独立。热力图改用本地时间，并按照当前 7 日视图中最大的单小时 Token 桶计算九档相对对数色阶，在跨度较大时仍保留可见差异；这个峰值不是账号额度上限。零值采用安静底色，标签跟随界面语言，光标悬浮提示信息即时出现，大数值自动使用 K/M 单位的简洁显示。

The total-cost card, seven-day card, projects, sessions, prompts, and heatmap all use the same local-log model pricing. The complete project breakdown therefore adds up to the displayed total. The seven-day card uses the previous seven completed local calendar days, while the cost alert uses the rolling latest 168 hours so current activity can trigger warnings. Analytics no longer requests or caches official Profile activity.

总成本卡片、7 日成本卡片、项目、会话、prompt 与热力图现全部采用相同的本机日志模型计价，因此完整项目明细相加会与显示的总成本一致。7 日成本卡片采用前 7 个完整本地自然日，成本预警采用滚动最近 168 小时，使当天活动能够及时触发告警。分析页不再请求或缓存官方 Profile 活动量。

The seven-day cost uses the previous seven completed local calendar days. The current day is excluded from that comparison until it is complete. The rolling alert and heatmap still include current activity.

7 日成本采用前 7 个完整本地自然日，当日结束前不纳入该项成本比较。滚动预警与热力图仍会包含当天活动。

| Before / 修改前 |
| --- |
| ![Analytics heatmap before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/analytics-heatmap-before.png) |

| After / 修改后 |
| --- |
| ![Analytics heatmap after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/analytics-heatmap-after.png) |

### 4. Cached account data and freshness / 账号缓存与数据新鲜度

When the app opens, locally stored accounts and their last quota snapshots render first while remote quota refresh starts concurrently. During first-load work, the UI distinguishes cached data from an in-progress refresh; after a successful refresh the freshness badge disappears. Failed or unavailable states remain visible, with a concise failure cause and the full error in hover text.

打开应用后，界面会先显示保存在本地的账号及上次额度快照，同时并发刷新远端额度。首次加载期间，界面会区分缓存数据与更新中状态；刷新成功后新鲜度提示自动隐藏。失败或暂无数据时仍保留提示，其中失败状态直接显示简短原因，并在悬停信息中保留完整错误。

| Before / 修改前 |
| --- |
| ![Account usage loading before](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/account-usage-loading-before.png) |

| After / 修改后 |
| --- |
| ![Account usage loading after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/account-usage-loading-after.png) |

### 5. Plus-to-Pro account resolution and membership metadata / Plus 升级 PRO 后的账号识别与会员信息

Account identity is now matched before mutable plan metadata is considered. If `auth.json` still reports Plus while the stored account and quota snapshot are already Pro, startup reuses the cached Pro record instead of creating an empty Plus variant. This keeps the last quota visible during the background refresh and lets the status bar select the current account correctly.

On startup and manual quota refresh, a known mismatch between the live quota plan and the ID-token plan can trigger one controlled token refresh, with a cooldown to avoid repeated credential rotation. The account detail card can show the future `chatgpt_subscription_active_until` claim as **reference-only** membership-expiry data; past values are hidden because this private claim can remain stale after an upgrade. If an ordinary refresh still returns a stale claim, full OAuth reauthorization is required. Reauthorizing the active account now also writes the new credentials to `auth.json`, preventing the next quota refresh from restoring the old snapshot.

账号会先按稳定身份匹配，再处理可变化的套餐元数据。如果 `auth.json` 仍显示 Plus，而本地账号及额度快照已经是 PRO，启动时会复用带缓存的 PRO 记录，不再创建空白 Plus 变体。这样后台刷新期间仍会显示上次额度，状态栏也能正确选中当前账号。

启动和手动刷新额度时，如果实时额度套餐与 ID token 套餐均已知且不一致，应用会在冷却时间约束下执行一次受控令牌刷新，避免反复轮换凭据。账号详情可将未来的 `chatgpt_subscription_active_until` 声明显示为**仅供参考**的会员到期时间；已过期数值会隐藏，因为该私有声明在套餐升级后可能仍然陈旧。若普通刷新仍返回旧声明，则需要完整 OAuth 重新授权。当前账号重新授权后，新凭据现在也会同步写入 `auth.json`，避免下一次额度刷新恢复旧快照。

The existing `main` UI already shows the available reset-card count and each card's expiry. This PR adds the separate reference-only membership expiry, completing the account-lifecycle information requested in Issue #136.

现有 `main` 界面已经显示可用重置卡数量及每张卡的过期时间；本 PR 补充独立的、仅供参考的会员到期时间，从而完整覆盖 Issue #136 请求的账号生命周期信息。

![Plus-to-Pro account resolution before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/refs/heads/codex/energy-optimization/docs/pr-assets/energy-optimization/account-resolution-before-after.png)

### 6. Optional release descriptions / 可选的更新说明

When bilingual changelog notes exist, the release workflow reuses them for GitHub Release and the in-app updater. Missing notes only produce a warning and generated fallback text; they do not block a release. Debug-only redacted auth diagnostics remain excluded from release builds.

存在双语更新日志时，发布流程会将其复用于 GitHub Release 与应用内更新弹窗；缺少说明时仅警告并生成兜底文字，不阻断发布。脱敏授权诊断仍只存在于调试构建中。

## Related issue / 关联 Issue

Closes #136

## Validation / 验证

- [x] Frontend lint and production build / 前端检查与生产构建：`npm run lint`, `npm run build`
- [x] Debug desktop application build / 调试桌面应用构建：`tauri build --debug --no-bundle`
- [x] Full Rust suite / 完整 Rust 测试：188 passed
- [x] Default 60-second incremental analytics A-B-B-A run with a growing log; all appended events were consumed / 默认每 60 秒增量分析 A-B-B-A 增长日志测试；全部新增事件均被读取
- [x] Quota refresh remained active during startup, scheduled refresh, and manual refresh / 启动、定时与手动刷新期间账号额度链路保持工作
- [x] Three real cold starts with `auth.json = Plus` and cached usage = PRO kept two accounts and displayed the cached 28% quota immediately / 在 `auth.json = Plus`、缓存用量 = PRO 的真实环境中连续冷启动三次，始终保持两个账号并立即显示缓存额度 28%
- [x] Full OAuth reauthorization synchronized the active `auth.json` and stored account snapshot and returned a current future membership-expiry claim / 完整 OAuth 重新授权后，当前 `auth.json` 与账号库快照保持一致，并获得当前有效的未来会员到期声明
- [x] Fork replay, local-time heatmap, cached-account freshness, fixed 60-second refresh, concise failure reasons, and no Analytics page-entry refresh regressions / fork 去重、本地时区热力图、账号缓存新鲜度、固定 60 秒刷新、简短失败原因及取消分析页进页刷新回归
- [x] `last_token_usage` accounting stays consistent across the Token summary, detailed analytics, and heatmap when cumulative snapshots repeat / 累计快照重复时，Token 汇总、详细分析与热力图仍一致按 `last_token_usage` 统计
- [x] Nine-level logarithmic heatmap scaling keeps zero buckets distinct and maps representative low, medium, and peak values deterministically / 九档对数热力图色阶保持零值独立，并稳定映射低、中、高及峰值样本
- [x] Seven-day local cost uses completed local calendar days, while the rolling 168-hour alert still includes current activity / 本机 7 日成本采用完整本地自然日，滚动 168 小时预警仍包含当前活动
- [x] Total, seven-day, project, session, prompt, and heatmap costs share the same local-log pricing; no official Profile request or cache remains / 总成本、7 日成本、项目、会话、prompt 与热力图共用本机日志计价，不再保留官方 Profile 请求或缓存
