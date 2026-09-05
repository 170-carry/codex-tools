## English

### Summary

Add Astra pricing and calculate historical Terra/Luna/Sol costs using the rates effective when each usage event occurred.

### Problem and approach

Astra previously used fallback rates. A single fixed rate per model also mispriced history spanning a price change: applying the latest rate to older events retroactively changed their estimates.

Pass each usage event's timestamp into rate selection, so events within the same session can use different historical prices. The implemented rates are listed below in USD per million tokens; each tuple is **input / cached input / output**, and cutovers are at 00:00 UTC.

| Model | Before cutover | On/after cutover |
| --- | --- | --- |
| Astra | — | 10 / 1 / 50 |
| Terra — July 30, 2026 | 2.5 / 0.25 / 15 | 2 / 0.2 / 12 |
| Luna — July 30, 2026 | 1 / 0.1 / 6 | 0.2 / 0.02 / 1.2 |
| Sol — August 21, 2026 | 5 / 0.5 / 30 | 4 / 0.4 / 20 |

Astra has a dedicated rate rather than a date cutover. Recognized model aliases and dated variants use the corresponding model rates.

Upgrade the cost cache to version 10 so previously cached estimates are rebuilt with the corrected pricing.

![Astra session estimates / Astra 会话费用估算](https://github.com/user-attachments/assets/50bb53ae-d734-43a2-b6a7-b7c0d1512e5a)

### Validation

Regression tests cover both sides of the price cutovers, cached-input calculations, model aliases, a session spanning a cutover, and rejection of older cache versions.

For example, 1M input tokens including 0.5M cached tokens plus 1M output tokens produce a Sol estimate of $32.75 before its cutover and $22.20 afterward; the same usage produces $55.50 for Astra.

270 Rust library tests passed; formatting and diff checks passed.

Estimates use standard short-context API rates. Pricing references: [Astra](https://developers.openai.com/api/docs/models/gpt-6-astra), [Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), [changelog](https://developers.openai.com/api/docs/changelog).

## 中文

### 概述

新增 Astra 计价，并按每条用量事件发生时的有效价格，计算 Terra、Luna 和 Sol 的历史费用。

### 问题与方案

此前 Astra 使用兜底价格；每个模型只使用一组固定价格，也会使跨调价日期的历史记录估算错误：将最新价格应用于旧事件，会追溯改变旧记录的估算。

现在将每条用量事件的时间传入价格选择逻辑，同一会话内的事件也可以使用不同历史价格。下表为本次实现的价格，单位为美元／百万 Token，各组依次为 **输入／缓存输入／输出**；调价分界采用 UTC 00:00。

| 模型 | 调价前 | 调价当日及之后 |
| --- | --- | --- |
| Astra | — | 10 / 1 / 50 |
| Terra — 2026-07-30 | 2.5 / 0.25 / 15 | 2 / 0.2 / 12 |
| Luna — 2026-07-30 | 1 / 0.1 / 6 | 0.2 / 0.02 / 1.2 |
| Sol — 2026-08-21 | 5 / 0.5 / 30 | 4 / 0.4 / 20 |

Astra 为新增专用价格，不设调价分界。已识别的模型别名和带日期版本名使用对应模型价格。

成本缓存升级至版本 10，使此前缓存的估算按修正后的计价逻辑重新生成。

![Astra session estimates / Astra 会话费用估算](https://github.com/user-attachments/assets/50bb53ae-d734-43a2-b6a7-b7c0d1512e5a)

### 验证

回归测试覆盖调价分界两侧、缓存输入计算、模型别名、跨调价时刻的同一会话，以及旧版缓存拒绝读取。

例如，100 万输入 Token（其中 50 万为缓存输入）加 100 万输出 Token，Sol 在调价前估算为 $32.75，调价后为 $22.20；相同用量的 Astra 估算为 $55.50。

270 项 Rust 库测试通过，格式与差异检查通过。

费用按标准短上下文 API 价格估算。定价参考：[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra)、[Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol)、[更新日志](https://developers.openai.com/api/docs/changelog)。
