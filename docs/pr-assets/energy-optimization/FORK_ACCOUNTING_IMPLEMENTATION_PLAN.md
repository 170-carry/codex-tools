# Fork-aware local Token accounting plan / Fork 感知的本机 Token 统计实施计划

## Goal / 目标

Make local Token summaries, cost analytics, project/session breakdowns, and the heatmap count only activity owned by each rollout. Nested forks, replayed metadata, repeated rate-limit snapshots, missing parents, active-file growth, and cache reuse must not silently inflate usage.

确保本机 Token 汇总、成本分析、项目/会话明细及热力图只统计各 rollout 自身拥有的活动。嵌套 fork、重复元数据、额度快照重复广播、父日志缺失、活跃文件增长及缓存复用均不得静默放大用量。

## Invariants / 不变量

1. The first non-empty physical record must be the canonical `session_meta`; identity fields never change while parsing later records.
2. `payload.id`, `payload.session_id`, and the filename UUID are stored separately. The filename is validation evidence, not a replacement identity.
3. `forked_from_id` represents history inheritance. `parent_thread_id` represents agent ownership and does not imply copied history by itself.
4. Raw parsed records remain immutable. Inherited ranges and owned events are derived metadata.
5. Fork matching uses the direct parent's raw record stream. A parent's own inherited range is not removed before matching its child.
6. Repeated cumulative Token snapshots contribute zero. Monotonic increases contribute component-wise deltas. Counter decreases are diagnosed rather than guessed.
7. Missing or ambiguous lineage is surfaced as unresolved instead of silently counted as confirmed usage.

1. 第一个非空物理记录必须是当前文件的规范 `session_meta`；后续记录不得修改身份字段。
2. 分别保存 `payload.id`、`payload.session_id` 与文件名 UUID；文件名仅用于校验，不能替代规范身份。
3. `forked_from_id` 表示历史继承；`parent_thread_id` 表示 Agent 归属，本身不代表复制了历史。
4. 原始解析记录保持不可变；继承区间与自有事件属于派生元数据。
5. fork 匹配使用直接父会话的原始记录流；不得先删除父会话自身继承的区间再与子会话比较。
6. 重复累计 Token 快照贡献为零；单调增加时按各 Token 分量计算差值；计数器下降时记录诊断，不进行猜测。
7. 父关系缺失或边界不明确时显示未解析状态，不把不确定数据静默算作已确认用量。

## Execution phases / 执行阶段

### Phase 1 — Regression fixture and baseline / 阶段 1——回归样本与基线

- Add a small synthetic `A → B → C` fixture that reproduces copied `session_meta`, rewritten outer timestamps, repeated Token snapshots, and child-owned suffix events.
- Record expected raw, inherited, owned, and confirmed Token totals.
- Add the current failure as a regression before changing production code.

- 增加小型合成 `A → B → C` 样本，复现复制的 `session_meta`、改写的外层时间戳、重复 Token 快照及子会话自有后缀。
- 固化原始、继承、自有与已确认 Token 预期值。
- 修改生产代码前先加入能够复现当前故障的回归测试。

### Phase 2 — Immutable identity hotfix / 阶段 2——不可变身份热修

- Parse the canonical header once in both the Token summary and detailed analytics paths.
- Preserve later metadata as embedded history without overwriting canonical identity.
- Validate filename/thread/session identifiers and quarantine untrusted headers from lineage matching.
- Bump the persisted analytics cache version.

- 在 Token 汇总与详细分析两条链路中仅解析一次规范文件头。
- 后续元数据作为嵌入历史保留，但不得覆盖规范身份。
- 校验文件名、thread 与 session 标识；不可信文件头不得参与关系匹配。
- 升级持久化分析缓存版本。

### Phase 3 — Direct-parent inheritance ownership / 阶段 3——直接父会话继承归属

- Build typed history-inheritance and agent-ownership edges.
- Prefer explicit history positions when present; for legacy logs, stream-match normalized parent and child records.
- Store inherited ranges and owned event indexes without mutating raw events.
- Mark missing, cyclic, conflicting, or unverifiable lineage as unresolved.

- 建立有类型的历史继承边与 Agent 归属边。
- 存在明确历史位置时优先使用；legacy 日志采用规范化父子记录流的流式匹配。
- 保存继承区间及自有事件索引，不修改原始事件。
- 父日志缺失、关系成环、冲突或无法验证时标记为未解析。

### Phase 4 — Cumulative-delta usage / 阶段 4——累计差值用量

- Audit the local corpus for unchanged totals, counter decreases, missing totals, and schema transitions.
- Calculate input, cached-input, output, reasoning-output, and total deltas only after ownership is known.
- Feed the same confirmed deltas into summaries, costs, projects, sessions, prompts, and hourly buckets.
- Keep unresolved usage separate from confirmed totals.

- 审计本机日志中累计值不变、计数器下降、累计值缺失及格式变化情况。
- 仅在归属确定后计算 input、cached-input、output、reasoning-output 与 total 的差值。
- 汇总、成本、项目、会话、prompt 及小时桶统一使用同一份已确认差值。
- 未解析用量与已确认总量分开保存。

### Phase 5 — Cache and large-file safety / 阶段 5——缓存与大文件安全

- Version parser, canonicalizer, lineage, and usage algorithms in cache dependencies.
- Invalidate child ownership when its parent digest changes or reappears.
- Bound active-file reads to a captured file length and verify truncation/replacement afterward.
- Use streaming hashes and bounded memory for large copied histories.

- 在缓存依赖中记录解析器、规范化器、关系与用量算法版本。
- 父日志摘要变化或重新出现时，使子会话归属缓存失效。
- 按读取前记录的文件长度限制活跃日志读取，并在完成后检查截断或替换。
- 对大型复制历史使用流式哈希及受限内存。

### Phase 6 — Validation and local PR draft / 阶段 6——验证与本地 PR 草稿

- Run focused regressions, the full Rust suite, frontend checks, production build, and real-log recomputation.
- Confirm the observed fork spike disappears without suppressing owned child activity.
- Measure initial and incremental scan time, attributed reads, and peak memory.
- Update only the local bilingual PR draft with verified behavior and limitations.

- 运行定向回归、完整 Rust 测试、前端检查、生产构建及真实日志复算。
- 确认已观察到的 fork 峰值消失，同时不抑制子会话自有活动。
- 测量首次及增量扫描时间、归因读取和峰值内存。
- 仅使用已验证的行为与边界更新本地双语 PR 草稿。

## Execution result / 执行结果

- All six phases are implemented locally. No commit, push, remote PR update, or GitHub mutation was performed.
- The current 6.2 GB nested fork contains replay-time inserted records and regenerated defaults. Bounded record-stream re-synchronization identified 156,799 inherited child records and stopped at the observed `task_complete` branch boundary; the following parent and child tasks remain independently owned.
- Recomputing 248 local logs reduced the false maximum heatmap bucket from about 4.90B to 109.7M Tokens. The summary and detailed analytics reported the same one unresolved old fork and one counter-reset anomaly.
- In the unoptimized test build, cold Token and detailed-analytics scans took about 197 and 228 seconds. With the same in-process cache and an actively growing log, incremental rescans took about 226 and 440 milliseconds. These debug timings do not claim release-build startup latency.
- Rust formatting/checks, 198 Rust tests, 8 refresh-error tests, ESLint, and the frontend production build passed. The production build retains the pre-existing Vite chunk-size warning.
- The local bilingual PR draft, changelog, and release-note preview now describe cumulative-delta accounting and verified direct-parent replay exclusion instead of the obsolete `last_token_usage` summation rule.

- 六个阶段均已在本地完成；未执行 commit、push、远端 PR 更新或任何 GitHub 写入。
- 当前 6.2 GB 嵌套 fork 包含回放时插入的记录及重新生成的默认字段。有限窗口记录流重新同步识别出 156,799 条子日志继承记录，并在观察到的 `task_complete` 分叉边界停止；其后的父子任务仍分别归属各自会话。
- 重新计算 248 个本机日志后，错误的热力图最大格从约 4.90B 降至 109.7M Token。摘要与详细分析均报告同一个旧缺失父 fork 及同一个计数器回退异常。
- 在未优化测试构建中，Token 摘要与详细分析冷扫描分别约为 197 秒和 228 秒；复用同一进程缓存且日志仍在增长时，增量复扫分别约为 226 毫秒和 440 毫秒。这些调试构建数据不用于宣称正式版启动耗时。
- Rust 格式与检查、198 项 Rust 测试、8 项刷新错误测试、ESLint 及前端生产构建均通过；生产构建仍保留既有的 Vite chunk-size 警告。
- 本地双语 PR 草稿、更新日志及应用内更新说明预览已改为描述累计差值统计与已验证的直接父会话回放排除，不再使用过时的 `last_token_usage` 累加规则。

## Completion criteria / 完成标准

- Nested forks retain canonical child identity from the first header.
- Replayed parent history contributes zero to the child.
- Repeated rate-limit snapshots contribute zero.
- All local analytics surfaces agree at the same scope and time boundary.
- Ambiguous lineage is visible and never silently presented as confirmed usage.
- Existing append-only incremental refresh remains correct and materially cheaper than a full rescan.

- 嵌套 fork 始终保留首条文件头中的规范子会话身份。
- 父会话重复历史对孩子会话的贡献为零。
- 额度快照重复广播的贡献为零。
- 所有本机分析界面在相同范围和时间边界下结果一致。
- 不明确的继承关系可见，且不会静默显示为已确认用量。
- 现有仅追加增量刷新保持正确，并继续显著低于完整重扫成本。
