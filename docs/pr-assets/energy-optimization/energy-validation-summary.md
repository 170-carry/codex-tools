# Codex Tools energy validation report / Codex Tools 能耗验证报告

Generated from one unattended runner. Raw artifacts remain private and are not copied into the repository.

由一个无人值守运行器生成。原始产物保持私有，不复制进仓库。

## English

### Conclusion

- The required hidden-window A-B-B-A, visible-window A-B-B-A, stale-cache startup, real low-frequency, and synthetic 10/30-account scenarios completed with raw artifacts retained outside the repository.
- In the controlled hidden-window scenario, the modified build substantially reduced coalition and primary-process CPU work, attributed reads, and Apple Energy Impact. The table reports both valid rounds and does not claim whole-machine power or battery-life improvement.
- Visible-window polling, startup remote-quota refresh, and forced manual refresh remained active; lower hidden-window work was not obtained by disabling account quota refresh.
- The startup result separates earlier cached-account usability from later remote quota completion; it does not describe the remote API itself as faster.
- The local mock refreshed all synthetic accounts, exercised a first-candidate retry, and recorded 0 external `chatgpt.com` requests. 2 other proxied request(s), such as the normal GitHub update check, are reported separately.

### Hidden window (30 min A-B-B-A)

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 12571.96 (12467.74–12676.18) | 123.35 (119.56–127.14) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 14544.36 (14393.73–14694.99) | 21.30 (19.32–23.27) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 316.27 (297.28–335.25) | 1142.12 (1121.67–1162.57) |
| Attributed reads B/s / 归因读取字节/秒 | 296127033.60 (295383731.10–296870336.10) | 87.75 (18.00–157.50) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 661.95 (658.30–665.60) | 0.32 (0.30–0.35) |

### Visible account window (20 min A-B-B-A, one manual refresh at minute 10)

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 20714.36 (12855.67–28573.05) | 254.12 (246.89–261.35) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 22029.15 (14835.15–29223.14) | 65.55 (62.55–68.55) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 449.76 (389.80–509.72) | 815.62 (781.68–849.57) |
| Attributed reads B/s / 归因读取字节/秒 | 256494369.10 (194903909.60–318084828.60) | 335.95 (145.20–526.70) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 508.59 (326.18–690.99) | 3.74 (3.56–3.91) |

Every valid visible-window run contains multiple distinct remote `fetchedAt` values and a post-click timestamp advance. Raw store samples and event timelines are retained for audit.

### Stale-cache startup (three runs per build)

| Build | Window visible median | Meter label after window median | Remote quota complete median | Token summary complete median | Analytics complete median |
| --- | ---: | ---: | ---: | ---: | ---: |
| `main` | 1.35 s | 0.11 s | 9.80 s | 23.98 s | 15.37 s |
| Modified / 修改版 | 1.25 s | 0.15 s | 3.72 s | 13.34 s | 15.41 s |

The second timing starts only after the account window is detectable and ends when the `5h usage` meter label appears. It is not the time from process launch to current quota data; the remote-quota column is the relevant end-to-end measurement for that stage.

### Real low-frequency use (two hours per build)

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 11945.64 (11945.64–11945.64) | 364.64 (364.64–364.64) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 14095.01 (14095.01–14095.01) | 293.72 (293.72–293.72) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 1086.89 (1086.89–1086.89) | 562.21 (562.21–562.21) |
| Attributed reads B/s / 归因读取字节/秒 | 296574612.90 (296574612.90–296574612.90) | 4427544.40 (4427544.40–4427544.40) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 650.86 (650.86–650.86) | 13.12 (13.12–13.12) |

Only one real ChatGPT account was available in the isolated store, so the two-hour daily-use runs validate account/quota/status-item behavior but record the requested real-account switch as unavailable.

### Synthetic scalability

- Release UI/local operations: 4 grouped A/B runs across 10 and 30 Relay accounts. Each run used the UI to select and switch an account and included active-account plus profile/config store updates, but excluded real ChatGPT authentication, remote quota requests, and Codex relaunch.
- Local mock quota refresh on the modified build: 10 accounts: 61 requests, maximum concurrency 2, chatgpt.com external 0, other proxied 1; 30 accounts: 211 requests, maximum concurrency 4, chatgpt.com external 0, other proxied 1. These runs covered startup, manual, and the next scheduled refresh with a fixed 0.5-second local delay, including retry and concurrency behavior but not real Internet/API latency.
- Token-log analytics scans one global local JSONL corpus rather than one corpus per account. It was validated separately against the 229-file corpus and was not multiplied in the 10/30-account fixtures.
- These results are not an energy estimate for 10 or 30 real Plus/Pro accounts.

### Validation and limitations

- Final checks passed: git diff check, frontend lint/build, Rust formatting, and the complete Rust test suite.
- 8 invalid or intentionally stopped run(s) remain listed in the manifest and were not mixed into valid aggregates.
- Activity Monitor 12-hour power is not used as a strict A/B metric, and whole-machine CPU/GPU/ANE power is excluded because unrelated load was not controlled.

## 中文

### 结论

- 已完成隐藏窗口 A-B-B-A、可见窗口 A-B-B-A、陈旧缓存启动、真实低频使用，以及 10/30 个合成账号场景；原始产物保存在仓库外。
- 在受控隐藏窗口场景中，修改版显著降低了 coalition 与主进程 CPU 工作、归因读取和 Apple Energy Impact。表格同时纳入两轮有效数据，不把它表述为整机功率或续航提升。
- 可见窗口轮询、启动后的远端额度更新和强制手动刷新仍然有效；后台负载下降并非通过停用账号额度刷新获得。
- 启动数据把“缓存账号更早可操作”与“远端额度随后完成”分开报告，不把远端 API 描述为变快。
- 本地 mock 已刷新全部合成账号、覆盖首个候选地址失败后的重试，并记录到 0 次外部 `chatgpt.com` 请求；另有 2 次 GitHub 更新检查等其他代理请求，已单独列出。

### 隐藏窗口（30 分钟 A-B-B-A）

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 12571.96 (12467.74–12676.18) | 123.35 (119.56–127.14) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 14544.36 (14393.73–14694.99) | 21.30 (19.32–23.27) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 316.27 (297.28–335.25) | 1142.12 (1121.67–1162.57) |
| Attributed reads B/s / 归因读取字节/秒 | 296127033.60 (295383731.10–296870336.10) | 87.75 (18.00–157.50) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 661.95 (658.30–665.60) | 0.32 (0.30–0.35) |

### 账号窗口可见（20 分钟 A-B-B-A，第 10 分钟手动刷新一次）

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 20714.36 (12855.67–28573.05) | 254.12 (246.89–261.35) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 22029.15 (14835.15–29223.14) | 65.55 (62.55–68.55) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 449.76 (389.80–509.72) | 815.62 (781.68–849.57) |
| Attributed reads B/s / 归因读取字节/秒 | 256494369.10 (194903909.60–318084828.60) | 335.95 (145.20–526.70) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 508.59 (326.18–690.99) | 3.74 (3.56–3.91) |

每轮有效可见窗口测试都包含多个不同的远端 `fetchedAt`，手动刷新后时间戳再次推进；原始 store 采样与事件时间线均保留以供审计。

### 陈旧缓存启动（每个构建 3 次）

| 构建 | 主窗口可见中位数 | 窗口出现后用量栏标签就绪中位数 | 远端额度完成中位数 | Token 汇总完成中位数 | 分析完成中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `main` | 1.35 s | 0.11 s | 9.80 s | 23.98 s | 15.37 s |
| Modified / 修改版 | 1.25 s | 0.15 s | 3.72 s | 13.34 s | 15.41 s |

第二列计时只在账号窗口已可检测后开始，到“5 小时使用率”标签出现时结束；它不是从进程启动到当前额度数据加载完成的时间。若要观察该阶段的端到端时间，应看“远端额度完成”一列。

### 真实低频使用（每个构建 2 小时）

| Metric / 指标 | `main` median (range) / 中位数（范围） | Modified median (range) / 修改版中位数（范围） |
| --- | ---: | ---: |
| Coalition CPU ms/min / coalition CPU 毫秒/分钟 | 11945.64 (11945.64–11945.64) | 364.64 (364.64–364.64) |
| Main-process CPU ms/min / 主进程 CPU 毫秒/分钟 | 14095.01 (14095.01–14095.01) | 293.72 (293.72–293.72) |
| Coalition wakeups/min / coalition 唤醒/分钟 | 1086.89 (1086.89–1086.89) | 562.21 (562.21–562.21) |
| Attributed reads B/s / 归因读取字节/秒 | 296574612.90 (296574612.90–296574612.90) | 4427544.40 (4427544.40–4427544.40) |
| Apple Energy Impact / s / Apple Energy Impact 每秒评分 | 650.86 (650.86–650.86) | 13.12 (13.12–13.12) |

隔离账号库中只有 1 个真实 ChatGPT 账号，因此两小时日常使用轮次可验证账号、额度与状态栏链路，但真实双账号切换被明确记录为不可执行。

### 合成账号扩展性

- Release UI/本地操作：对 10 与 30 个 Relay 账号执行 4 组 A/B 测试。每轮均通过界面选中并切换账号，包含当前账号及 profile/config 存储更新，但不包含真实 ChatGPT 认证、远端额度请求或 Codex 重启。
- 修改版的本地 mock 额度刷新：10 个账号为 61 次请求、最大并发 2、chatgpt.com 外部请求 0、其他代理请求 1；30 个账号为 211 次请求、最大并发 4、chatgpt.com 外部请求 0、其他代理请求 1。测试覆盖启动、手动和下一轮定时刷新，并使用固定延迟 0.5 秒的本地服务，包含重试与并发行为，但不代表真实互联网/API 延迟。
- Token 日志分析扫描的是一份全局本地 JSONL 日志集，而不是每个账号各扫描一份；该链路已在 229 文件日志集上单独验证，没有在 10/30 账号夹具中按账号数放大。
- 这些结果不代表 10 个或 30 个真实 Plus/Pro 账号的绝对能耗。

### 验证与限制

- 最终检查全部通过：git diff 检查、前端 lint/build、Rust 格式检查和完整 Rust 测试。
- manifest 保留 8 个无效或主动中止的轮次，汇总时没有与有效数据混用。
- 活动监视器“12 小时电源”不作为严格 A/B；由于无法控制无关负载，整机 CPU/GPU/ANE 功率不用于版本差异结论。
