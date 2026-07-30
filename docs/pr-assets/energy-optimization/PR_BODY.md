<!-- PR title / PR 标题: [EN/ZH] Improve the macOS status bar and reduce background energy use / 优化 macOS 状态栏并降低后台耗电 -->

> **Draft status / 草稿状态:** This local draft describes the current worktree, including changes not yet pushed to the PR. / 本地草稿按当前工作树编写，其中包含尚未推送到 PR 的改动。

## Summary / 摘要

- **Reduce background resource use / 降低后台资源占用:** remove inference keepalive work and replace repeated full log scans with one-minute incremental local-log refreshes.
- **Improve the macOS status item / 优化 macOS 状态栏:** use the color app icon, add practical display modes and labels, and fully hide the item when disabled.
- **Make startup account data clearer / 改善启动时的账号信息:** show the last saved account and quota data before the network refresh completes, report concise refresh failures, and fix Plus-to-Pro account selection.
- **Make local analytics consistent / 统一本机分析口径:** use the same Token and cost calculation across summaries, projects, sessions, prompts, and the heatmap.
- **Add reference-only membership expiry / 增加仅供参考的会员到期时间:** show a future expiry claim when it is available.

## Changes / 改动

### 1. macOS status item and usage labels / macOS 状态栏与用量标签

The macOS status item now uses the color application icon, defaults to one-week remaining usage, optionally shows `5h / 1w` labels, and disappears completely when “Hidden” is selected. Account meters keep the semantic `5h` and `1w` labels even when both periods currently report the same value.

macOS 状态栏现在使用彩色应用图标，默认显示一周剩余用量，可选显示 `5h / 1w` 标签，并在选择“不显示”时完整隐藏。即使两个周期当前数值相同，账号用量栏仍分别标注 `5h` 与 `1w`。

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Status item before](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-before.png) | ![Status item after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/status-item-after.png) |

| Before / 修改前 |
| --- |
| ![Status settings before](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-before.png) |

| After / 修改后 |
| --- |
| ![Status settings after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/status-settings-after.png) |

| Before / 修改前 | After / 修改后 |
| --- | --- |
| ![Usage labels before](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-before.png) | ![Usage labels after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/usage-labels-after.png) |

### 2. Faster, clearer account startup / 更快、更清晰的账号启动体验

On a returning installation, the app renders locally stored accounts and the last quota snapshot before the remote refresh finishes. The first background refresh is marked as in progress; a successful refresh removes the badge, while failures remain visible with a short reason and the complete error on hover. A clean installation without a saved snapshot still waits for the first remote result, and cached values may be temporarily stale until that refresh completes.

对于已有本地数据的安装，应用会在远端刷新完成前显示已保存的账号和上次额度快照。首次后台刷新会显示更新中状态；成功后提示消失，失败时则保留简短原因，并可悬停查看完整错误。全新安装若没有历史快照，仍需等待首次远端结果；缓存值在刷新完成前也可能暂时陈旧。

| Before / 修改前 |
| --- |
| ![Account usage loading before](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/account-usage-loading-before.png) |

| After / 修改后 |
| --- |
| ![Account usage loading after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/account-usage-loading-after.png) |

### 3. Plus-to-Pro account and authorization recovery / Plus 升级 PRO 后的账号与授权恢复

Account matching now prioritizes stable identity over mutable plan metadata. If `auth.json` still says Plus while the saved account and quota snapshot are already PRO, startup reuses that PRO record, keeps its last quota visible, and lets the status item select the current account correctly instead of creating an empty Plus variant.

账号匹配现在优先使用稳定身份，而不是可变化的套餐字段。如果 `auth.json` 仍显示 Plus，而已保存的账号及额度快照已经是 PRO，启动时会复用该 PRO 记录、继续显示上次额度，并让状态栏正确选中当前账号，而不是创建空白 Plus 变体。

When a known live quota plan conflicts with the ID-token plan, startup or a manual quota refresh may perform one cooldown-protected token refresh. Full OAuth reauthorization is still required when ordinary refreshes cannot update stale identity claims; reauthorizing the active account now also synchronizes the new credentials back to `auth.json`.

当已知的实时额度套餐与 ID token 套餐不一致时，启动或手动刷新额度可以在冷却限制下执行一次令牌刷新。若普通刷新仍无法更新陈旧的身份声明，仍需完整 OAuth 重新授权；当前账号重新授权后，新凭据也会同步写回 `auth.json`。

![Plus-to-Pro account resolution before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/account-resolution-before-after.png)

### 4. Reference-only membership expiry / 仅供参考的会员到期时间

The account detail card shows a future `chatgpt_subscription_active_until` ID-token claim when available. Missing or past values are not presented as a valid expiry date, and the UI labels the field **for reference only** because this private claim can remain stale after a plan change. This is separate from the existing reset-card count and reset-card expiry display requested in Issue #136.

账号详情会在存在有效未来数值时显示 ID token 中的 `chatgpt_subscription_active_until`。缺失或已过去的数值不会作为有效到期时间展示；由于该私有声明在套餐变化后可能陈旧，界面明确标注其**仅供参考**。它与 Issue #136 中已有的重置卡数量及重置卡过期时间相互独立。

| Before / 修改前 |
| --- |
| ![Controlled membership expiry comparison before](https://raw.githubusercontent.com/Nonex111/codex-tools/44e52c7c387954e0fdb8aa3a8b9f304da2cfd3fa/docs/pr-assets/energy-optimization/membership-expiry-controlled-before.png) |

| After / 修改后 |
| --- |
| ![Controlled membership expiry comparison after](https://raw.githubusercontent.com/Nonex111/codex-tools/44e52c7c387954e0fdb8aa3a8b9f304da2cfd3fa/docs/pr-assets/energy-optimization/membership-expiry-controlled-after.png) |

### 5. Consistent local Token analytics and heatmap / 一致的本机 Token 分析与热力图

The Token summary and detailed analytics now derive actual increments from cumulative Token snapshots. An unchanged snapshot contributes zero; a monotonic increase contributes only its component-wise delta; counter resets remain visible as anomalies instead of being guessed. Total, seven-day, project, session, prompt, and heatmap values all consume these same confirmed local-log deltas and the same model-pricing estimate, so the complete project breakdown adds up to the displayed total.

Token 汇总与详细分析现在根据累计 Token 快照计算实际增量：累计值未变化时计为零，单调增长时仅计算各 Token 分量的差值，计数器回退则保留为异常而不猜测用量。总量、7 日、项目、会话、prompt 与热力图统一使用这份已确认的本机日志增量及同一套模型计价估算，因此完整项目明细可与显示的总成本相加一致。

Forked files keep the immutable identity from their first physical `session_meta`. History inheritance (`forked_from_id`) is separated from agent ownership (`parent_thread_id`), and only a verified direct-parent record range is excluded. The matcher tolerates small replay-time record insertions, omissions, regenerated IDs, and known default fields, but stops conservatively at the branch boundary. Missing, ambiguous, cyclic, or otherwise unverifiable lineage is excluded from confirmed totals and reported as unresolved.

fork 文件始终保留首条物理 `session_meta` 中的不可变身份。历史继承（`forked_from_id`）与 Agent 归属（`parent_thread_id`）分开处理，仅排除经验证的直接父会话记录区间。匹配器可以容忍回放时少量记录插入或缺失、重新生成的 ID 及已知默认字段，但会在真实分叉边界保守停止；父日志缺失、关系冲突、成环或无法验证时，不会计入已确认总量，并会显示为未解析。

These values are local-log Tokens and estimated API-equivalent costs—not official ChatGPT quota/Profile activity and not the API proxy's upstream `usage.total_tokens`. The seven-day card uses the previous seven completed local calendar days; the budget alert uses the rolling latest 168 hours so current activity can still trigger a warning.

这些数值是本机日志 Token 与 API 等值成本估算，不是 ChatGPT 官方额度/Profile 活动量，也不是 API 反代上游返回的 `usage.total_tokens`。7 日卡片采用前 7 个完整本地自然日；预算预警采用滚动最近 168 小时，使当天活动仍可触发告警。

The heatmap uses local time, immediate localized tooltips, compact K/M values, and nine logarithmic levels relative to the largest one-hour bucket in the current seven-day view. Zero remains visually distinct. The comparison below uses the same deterministic 168-hour fixture in both images; only the coloring algorithm changes.

热力图按本地时间显示，悬停提示即时出现并跟随界面语言，大数值采用 K/M 单位；颜色按照当前 7 日视图中最大单小时值分配九档对数相对色阶，零值保持独立。下方两张图使用同一份固定的 168 小时模拟数据，唯一变量是染色算法。

| Before / 修改前 |
| --- |
| ![Controlled heatmap simulation before](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/analytics-heatmap-controlled-before.png) |

| After / 修改后 |
| --- |
| ![Controlled heatmap simulation after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/analytics-heatmap-controlled-after.png) |

### 6. Incremental background analysis / 增量后台分析

The app no longer sends an inference keepalive. After the initial scan in each app process, unchanged log files reuse their parsed in-memory results and append-only files are read from their previous byte offset. Truncated, rewritten, or tail-mismatched files fall back to reparsing that file for correctness. The latest aggregate analytics snapshot is persisted for fast display after a restart; the per-file parsing index itself is rebuilt on the first scan of the new process.

应用不再发送推理保活请求。每次应用进程完成首次扫描后，未变化日志会复用内存中的逐文件解析结果，仅追加文件则从上次字节位置继续读取；文件被截断、重写或尾部校验不匹配时，会为保证正确性重新解析该文件。最新的聚合分析快照会持久化，供重启后快速显示；逐文件解析索引本身仍会在新进程首次扫描时重建。

Detailed local-log analytics refreshes every 60 seconds, and **Refresh analytics** remains available for an immediate update. Entering the Analytics page does not start another scan. This schedule affects only local session-log analysis; account quota refresh and API-proxy usage collection are independent. Nonessential foreground polling also pauses while the main window is hidden.

本机会话日志分析固定每 60 秒刷新一次，同时保留“刷新分析”用于立即更新；进入分析页不会再额外启动扫描。该周期只影响本机会话日志分析，账号额度刷新和 API 反代用量采集均为独立链路。主窗口隐藏时，非必要的前台轮询也会暂停。

![Background work before and after](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/energy-optimization/docs/pr-assets/energy-optimization/background-work-before-after.png)

In a same-Mac A-B-B-A comparison against clean v2.4.0 `main`, both builds used the same isolated 229-file log corpus and received one appended Token event per minute. The modified build consumed every appended event while substantially reducing recurring CPU work, attributed reads, and Apple Energy Impact:

在同一台 Mac 上与干净 v2.4.0 `main` 进行 A-B-B-A 对比时，两版使用相同的隔离 229 文件日志集，并每分钟追加一条 Token 事件。修改版读取了全部新增事件，同时显著降低了周期 CPU 工作、归因读取和 Apple Energy Impact：

| Scenario<br>场景 | Coalition CPU<br>ms/min | Attributed reads<br>归因读取字节<br>B/s | Apple Energy Impact/s |
| --- | ---: | ---: | ---: |
| Default one-minute refresh with a growing log<br>默认每分钟刷新（日志增长） | 12,692 → 336 | 291,028,172 → 30 | 664.13 → 5.02 |

These are macOS process-group attribution metrics, not whole-machine watt-hours. Wakeups did not improve in this short run, so this PR does not claim fewer wakeups.

这些指标是 macOS 对应用进程组的归因统计，不是整机瓦时。短时测试中的唤醒次数没有改善，因此本 PR 不主张唤醒次数降低。

### 7. Optional bilingual release descriptions / 可选的双语更新说明

When bilingual changelog notes exist, the release workflow reuses them for GitHub Release and the in-app updater. Missing notes produce a warning and generic fallback text but do not block a release. Debug-only redacted authorization diagnostics remain excluded from release builds.

存在双语更新日志时，发布流程会将其复用于 GitHub Release 与应用内更新弹窗。缺少说明时仅发出警告并使用通用兜底文字，不阻断发布。脱敏授权诊断仍只存在于调试构建中。

## Related issue / 关联 Issue

Closes #136

## Validation / 验证

- [x] Frontend lint, production build, Rust check, and full Rust suite (198 passed) / 前端检查、生产构建、Rust 检查及完整 Rust 测试（198 项通过）
- [x] Controlled growing-log A-B-B-A energy run; every appended event was consumed / 受控增长日志 A-B-B-A 能耗测试；全部新增事件均被读取
- [x] Real cold starts with stale Plus auth metadata and cached PRO usage retained both accounts and the cached quota / 使用陈旧 Plus 认证元数据及 PRO 缓存用量进行真实冷启动；两个账号及缓存额度均保留
- [x] OAuth reauthorization synchronized the active `auth.json`; quota success, timeout, network, authorization, rate-limit, service, and invalid-response states were exercised / OAuth 重新授权可同步当前 `auth.json`；额度成功、超时、网络、授权、限流、服务及响应异常状态均已覆盖
- [x] Deterministic tests cover immutable fork identity, nested direct-parent ownership, replay insertions/omissions, unchanged cumulative snapshots, counter resets, missing parents, parent reappearance, local-time buckets, cost windows, heatmap levels, and cache append/reparse/eviction behavior / 确定性测试覆盖不可变 fork 身份、嵌套直接父会话归属、回放插入与缺失、累计快照不变、计数器回退、父日志缺失与重新出现、本地时间分桶、成本时间窗口、热力图色阶及缓存追加、重解析与淘汰行为
- [x] A real 6.2 GB nested fork retained 156,799 inherited records up to the observed branch boundary; recomputing 248 logs reduced the false maximum heatmap bucket from 4.90B to 109.7M Tokens while retaining child-owned suffix activity. Debug cold scans took 197–228 seconds; active-file incremental rescans took 226–440 ms / 一个真实的 6.2 GB 嵌套 fork 在观察到的分叉边界前识别出 156,799 条继承记录；重新计算 248 个日志后，错误的热力图最大格从 4.90B 降至 109.7M Token，同时保留子会话自有后缀。调试构建冷扫描耗时 197–228 秒，活动文件增量复扫耗时 226–440 毫秒
- [x] Release-note extraction covers versioned notes, Unreleased fallback, and missing-note fallback without blocking the build / 更新说明提取覆盖指定版本、Unreleased 回退及缺失说明回退，均不阻断构建
