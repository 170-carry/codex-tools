## English

### Windows account switching

The renamed `app/ChatGPT.exe` in the `OpenAI.Codex` package was missed by desktop discovery and stopping logic, while CLI fallback could hide the original failure.

- Resolve the registered desktop executable and AUMID before changing accounts.
- Stop only verified desktop processes and descendants; protect Codex Tools itself.
- Verify desktop startup and preserve both desktop and CLI failure details.

### Account recovery

- Explain missing membership expiry and add a nearby re-login action, disabled during authentication.
- Preserve refresh HTTP status and prioritize server failures over fallback authorization errors.
- Add regression coverage for reauthorizing another account without replacing the current account snapshot.

![Account recovery on macOS / macOS 账号恢复入口](https://github.com/user-attachments/assets/02d16b85-f798-41e1-bb68-304cb0febe86)

### Validation

268 macOS Rust tests and 24 frontend tests passed; build and formatting checks passed. Lint: 0 errors, 4 existing warnings.

Draft: final Windows patch reconciliation and account-switch evidence remain pending.

## 中文

### Windows 账号切换

`OpenAI.Codex` 包内改名后的 `app/ChatGPT.exe` 未被旧发现和停止逻辑识别，CLI 回退错误还可能遮蔽原始失败原因。

- 切换账号前解析已注册桌面程序路径及 AUMID。
- 仅停止已核实的桌面进程及子树，保护 Codex Tools 自身。
- 检查桌面启动结果，保留桌面与 CLI 的完整失败原因。

### 账号恢复

- 解释会员到期时间缺失，并在旁边增加重新登录入口，授权期间禁用。
- 保留刷新 HTTP 状态，优先展示服务端错误，不让备用接口授权错误遮蔽它。
- 增加重新授权其他账号时保留当前账号快照的回归测试。

![Account recovery on macOS / macOS 账号恢复入口](https://github.com/user-attachments/assets/02d16b85-f798-41e1-bb68-304cb0febe86)

### 验证

268 项 macOS Rust 测试、24 项前端测试通过，构建与格式检查通过。Lint 无错误，保留 4 个既有警告。

Draft：待对齐 Windows 最终补丁并补齐账号切换验证证据。
