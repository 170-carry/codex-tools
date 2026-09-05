## English

### Root cause and Windows correction

The official `OpenAI.Codex` Store package now uses `app/ChatGPT.exe`. The old executable discovery and name-based stop logic missed that desktop; unavailable CLI fallback could then hide the original desktop error. Launch-target discovery also happened after stopping the desktop and changing the current profile.

- Resolve the registered package's executable and AUMID together; use the Windows system PowerShell path without modifying PATH.
- Prepare a desktop/CLI launch plan before stopping processes or writing the current profile.
- Restrict stopping to verified executable paths, current user/session and descendants; protect Codex Tools and its descendants and check process start times against PID reuse.
- Check the actual desktop executable remains alive after activation. Preserve desktop and CLI errors and distinguish a changed account from a failed relaunch.
- Keep regular ChatGPT clients and CLI executables distinct from the registered Codex GUI.

### Account recovery and refresh errors

Missing membership dates come from absent token metadata, not necessarily expired membership. Explain this beside the value and provide the existing re-login action, disabled while authentication is busy. Preserve refresh HTTP status and prioritize a primary server failure over a fallback authorization error. Add regression coverage for preserving the current account's snapshot when another account is reauthorized; that behavior already existed.

![Native macOS missing-expiry recovery action / macOS 实机到期时间缺失恢复入口](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/account-recovery-windows/docs/pr-assets/account-recovery/mac-membership-recovery.png)

Native combined preview captured September 5, 2026; cropped only, without masking or changing UI content. This demonstrates the visible action, not a completed OAuth flow or busy-state verification. It is not a Windows screenshot.

### Validation and draft status

- Isolated candidate on official main `1c53f5d`: macOS Rust library tests **268 passed**; frontend tests **24 passed**; production frontend build, formatting and diff checks passed. ESLint: 0 errors and 4 existing unused-disable warnings; existing bundle-size warning remains.
- The retrieved ROG implementation record reports **284 Windows tests passed, 1 ignored**, a successful release build, and explicit installed-desktop discovery with CLI and PowerShell absent from PATH. These are prior ROG results, not Windows validation of this combined candidate.
- Windows changes were replayed from the complete recorded ROG source edits. The resulting local Windows-only patch digest does not match the recorded final patch digest; exact final-artifact reconciliation remains outstanding. Do not infer byte identity from the passing local tests.
- The user reports real switching verification completed, but the retrievable record still ends at an interrupted switch attempt. Final before/after results and a Windows screenshot have not been delivered. This PR is **draft** pending reconciliation and host evidence, not a release-ready claim.
- The missing-expiry action is visible in the native preview; live OAuth and authentication-busy interaction are not newly exercised here.

### Scope

Eight source/test files: `account_service.rs`, `cli.rs`, `lib.rs`, `AccountsGrid.tsx`, `backendErrors.ts`, `accounts.css`, `usageRefreshError.ts`, and its test. Account UI/errors and Windows switching are separate commits. Excludes model pricing in #196 and quota/scanning fixes in #195. No credentials, account exports, runtime data, or installers are included. Re-login does not guarantee the provider supplies an expiry date.

## 中文

### 根因与 Windows 修复

官方 `OpenAI.Codex` 商店包现使用 `app/ChatGPT.exe`。旧的程序发现和按名称停止逻辑漏掉了该桌面程序，CLI 不可用时的回退错误又可能遮蔽原始桌面错误。启动目标解析也晚于停止桌面和修改当前 profile。

- 成对解析当前注册包的程序路径与 AUMID；使用 Windows 系统 PowerShell 路径，不修改 PATH。
- 停止进程、写入当前 profile 之前，先准备桌面/CLI 启动计划。
- 将停止范围限定于已核实程序路径、当前用户/会话及其子树；保护 Codex Tools 及其子树，并检查进程启动时间以防 PID 重用。
- 激活后检查真实桌面程序持续存活；保留桌面和 CLI 错误，区分账号已改变与桌面未能启动。
- 不混淆普通 ChatGPT 客户端、CLI 程序与注册的 Codex GUI。

### 账号恢复与刷新错误

会员日期缺失来自令牌元数据缺失，不必然代表会员已过期。在缺失值旁解释来源，并提供现有重新登录入口，授权忙碌时禁用。保留刷新 HTTP 状态，并优先呈现主接口服务端错误，不让备用接口授权错误遮蔽它。增加重新授权其他账号时保留当前账号快照的回归测试；该行为此前已存在。

![Native macOS missing-expiry recovery action / macOS 实机到期时间缺失恢复入口](https://raw.githubusercontent.com/Nonex111/codex-tools/codex/account-recovery-windows/docs/pr-assets/account-recovery/mac-membership-recovery.png)

2026 年 9 月 5 日原生组合预览版实机图；仅裁剪，没有遮挡或改动界面内容。证明恢复入口可见，不证明 OAuth 完成或忙碌状态已验证，也不是 Windows 截图。

### 验证与草稿状态

- 基于官方 main `1c53f5d` 的独立候选：macOS Rust 库测试 **268 项通过**，前端测试 **24 项通过**；前端生产构建、格式及差异检查通过。ESLint 无错误，保留 4 个既有 unused-disable 警告及既有 bundle 大小警告。
- 取回的 ROG 实现记录载明 **284 项 Windows 测试通过、1 项忽略**，release 构建成功，并在 PATH 不含 CLI 和 PowerShell 时通过显式安装发现测试。这是此前 ROG 的结果，不是本组合候选的 Windows 验证。
- Windows 修改依据完整的 ROG 源码编辑记录回放。当前本地 Windows-only patch 摘要与记录中的最终 patch 摘要不一致，仍需核对最终产物；本地测试通过不能证明字节一致。
- 用户已告知真实切换验证完成，但可读取记录仍结束于一次中断的切换操作，尚未收到最终前后记录和 Windows 截图。本 PR 保持 **Draft** 等待产物对齐与实机证据，不宣称可发布。
- 原生预览版中恢复入口已可见，本轮未新增真实 OAuth 和授权忙碌交互验收。

### 范围

八个源码/测试文件：`account_service.rs`、`cli.rs`、`lib.rs`、`AccountsGrid.tsx`、`backendErrors.ts`、`accounts.css`、`usageRefreshError.ts` 及其测试。账号 UI/错误与 Windows 切换分别提交。排除 #196 模型计价和 #195 额度/扫描修复。不包含凭据、账号导出、运行数据或安装包。重新登录不保证服务方补全到期时间。
