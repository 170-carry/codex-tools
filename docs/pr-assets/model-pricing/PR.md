## English

### Root cause

Unknown Astra events used generic fallback rates. A single rate per model also mispriced historical events across price changes, and old cached totals could retain stale estimates.

### Summary

- Recognize `gpt-6-astra`, case-insensitive names and hyphen-suffixed variants, using standard short-context rates of $10 input, $1 cached input and $50 output per million tokens.
- Apply historical Terra/Luna rates across July 30, 2026 and Sol rates across August 21, 2026. Sol changes from $5/$0.50/$30 to $4/$0.40/$20 (input/cached input/output per million tokens).
- Use the UTC day boundary when the source supplies a date but not an exact effective time.
- Move cost-cache version to 10 so versioned stale totals are rebuilt; add price-boundary and cache regression tests.

### Scope and dependency

![Native macOS Astra session estimates / macOS 实机 Astra 会话估算](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/model-pricing-astra-history/docs/pr-assets/model-pricing/mac-astra-sessions.png)

Native combined preview, September 5, 2026. Cropped to the session list without masking or changing any UI content or values; publication of project/session identifiers was approved by the user. This shows real display output, not an isolated-pricing proof or actual charges.

Only pricing, timestamp plumbing, cache version/source label and related tests in `src-tauri/src/token_usage.rs`. Exclude `matching_record_boundary` and its short-mismatch test, already covered by [PR #195](https://github.com/170-carry/codex-tools/pull/195). The integrated preview included that fix; the new PR must not duplicate it. Prefer validating the integrated result after #195 merges.

No new model dashboard, model-specific quota measurement, or per-model breakdown for mixed-model sessions is introduced. The session label remains the model with the most tokens, and the default table shows the 80 most expensive sessions.

### Validation

The isolated pricing candidate passed all 270 Rust library tests, formatting and diff checks. The earlier combined preview also passed the frontend build and 24 frontend tests. On macOS it rebuilt 333 local log files and displayed Astra search results. One unchanged Astra session's estimate moved from $3.60 to $27.17; this is a local illustrative check, not a benchmark or billing reconciliation.

The screenshot comes from the combined native preview, not a separate GUI build of this isolated candidate.

### Sources and limits

Sources checked September 5, 2026: [Astra](https://developers.openai.com/api/docs/models/gpt-6-astra), [Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), [changelog](https://developers.openai.com/api/docs/changelog), [Terra/Luna announcement](https://openai.com/index/advancing-the-price-performance-frontier-with-gpt-5-6/).

These are local-log API-equivalent estimates, not subscription charges. Long-context multipliers, cache-write pricing and Fast-tier pricing are not implemented. Generic fallback behavior for other unknown models is unchanged.

## 中文

### 根因

未识别的 Astra 事件使用通用兜底价格。每个模型只有一套价格也会使跨调价日期的历史事件估算错误，旧缓存还可能继续保留过期估算。

### 摘要

- 识别 `gpt-6-astra`、大小写变体及连字符后缀变体，按标准短上下文每百万 Token 输入 $10、缓存输入 $1、输出 $50 估算。
- 分别以 2026 年 7 月 30 日、8 月 21 日为界应用 Terra/Luna、Sol 历史价格。Sol 从 $5/$0.50/$30 调整为 $4/$0.40/$20，顺序为每百万 Token 输入/缓存输入/输出。
- 官方仅提供日期、没有精确生效时刻时，采用 UTC 当日零点。
- 成本缓存升级到版本 10，使旧版本缓存重建，并增加调价边界及缓存回归测试。

### 范围与依赖

![Native macOS Astra session estimates / macOS 实机 Astra 会话估算](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/model-pricing-astra-history/docs/pr-assets/model-pricing/mac-astra-sessions.png)

2026 年 9 月 5 日原生组合预览版实机图。仅裁剪至会话列表，没有遮挡或改动界面内容与数值；用户已确认允许公开项目/会话标识。此图展示真实界面结果，不代表独立计价验证或实际扣费。

仅包含 `src-tauri/src/token_usage.rs` 中定价、事件时间传递、缓存版本/来源说明及相关测试。排除 [PR #195](https://github.com/170-carry/codex-tools/pull/195) 已覆盖的 `matching_record_boundary` 与短序列不匹配测试。组合预览版包含该修复，但新 PR 不应重复提交；优先在 #195 合并后验证集成结果。

不新增模型专属看板、分模型额度测量或混合模型会话的分模型明细。会话标签仍取 Token 最多的模型，默认表格展示费用最高的 80 个会话。

### 验证

独立计价候选通过全部 270 项 Rust 库测试、格式与差异检查。此前组合预览版的前端构建及 24 项前端测试也通过。在 macOS 上重算了 333 个本机日志文件并显示 Astra 搜索结果。一个未变动的 Astra 会话估算从 $3.60 变为 $27.17；这只是本机示例检查，不是基准测试或账单对账。

截图来自原生组合预览版，不是本独立候选单独构建的 GUI。

### 来源与限制

2026 年 9 月 5 日核对：[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)、[Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol)、[更新日志](https://developers.openai.com/api/docs/changelog)、[Terra/Luna 公告](https://openai.com/index/advancing-the-price-performance-frontier-with-gpt-5-6/)。

这是本机日志的 API 等价估算，不是订阅扣费。尚未实现长上下文倍率、缓存写入及 Fast 档位计价。其他未知模型的通用兜底行为不变。
