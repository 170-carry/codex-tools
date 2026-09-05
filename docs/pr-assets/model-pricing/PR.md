## English

### Changes

Astra events previously used fallback rates, and fixed model rates mispriced historical events across price changes.

- Add `gpt-6-astra` pricing: $10 input / $1 cached input / $50 output per million tokens.
- Price Terra/Luna and Sol events using the rates effective on their event dates, with July 30 and August 21, 2026 cutovers.
- Upgrade cost-cache version to 10 to rebuild stale estimates.
- Add price-boundary, mixed-history and cache regression tests.

![Astra session estimates / Astra 会话费用估算](https://github.com/user-attachments/assets/50bb53ae-d734-43a2-b6a7-b7c0d1512e5a)

### Validation

270 Rust library tests passed; formatting and diff checks passed.

Estimates use standard short-context API rates. Sources: [Astra](https://developers.openai.com/api/docs/models/gpt-6-astra), [Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), [changelog](https://developers.openai.com/api/docs/changelog).

## 中文

### 修改内容

此前 Astra 使用兜底价格，固定模型价格也使跨调价日期的历史事件估算错误。

- 新增 `gpt-6-astra` 计价：每百万 Token 输入 $10／缓存输入 $1／输出 $50。
- 按事件日期应用 Terra/Luna、Sol 对应的历史价格，调价分界分别为 2026 年 7 月 30 日、8 月 21 日。
- 成本缓存升级至版本 10，重建旧估算。
- 增加调价边界、混合历史与缓存回归测试。

![Astra session estimates / Astra 会话费用估算](https://github.com/user-attachments/assets/50bb53ae-d734-43a2-b6a7-b7c0d1512e5a)

### 验证

270 项 Rust 库测试通过，格式与差异检查通过。

费用按标准短上下文 API 价格估算。来源：[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)、[Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol)、[更新日志](https://developers.openai.com/api/docs/changelog)。
