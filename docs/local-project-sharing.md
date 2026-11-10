# 本地项目保留与恢复

提供默认开启的 `preserveLocalProjects` 设置，用于在同一台电脑的本地 ChatGPT 账号之间保留 Codex 项目和对话归属；已保存的显式关闭设置保持不变。

## 桌面使用

1. 在设置的账号切换区域确认“切换时保留本地项目”的状态。新设置及缺失字段默认开启，已保存的关闭值保持关闭。关闭时保持原切换流程，不执行项目预览或迁移。
2. 选择目标 ChatGPT 账号。手动切换与智能切换使用同一流程；API/中转账号不支持此模式，会收到明确提示。
3. 切换前读取一次预览。目标需要首次合并确认时，弹窗分别显示项目数、项目中的会话关联数、保持未分组的会话数和保留的目标已有项目数。未分组会话不计入项目关联数。
4. 取消、关闭弹窗或按 Escape 均不执行切换。确认只适用于本次预览；即使目标不需要合并确认，也会将预览指纹传给后端。预览之后状态改变会拒绝执行，须重新预览。
5. 实际保留切换会先关闭 Codex，再完成事务写入。即使“切换后启动 Codex”关闭，仍需要先关闭 Codex。请先保存正在编辑的内容。

| 状态 | 含义 |
| --- | --- |
| `prepared` | 已准备保留数据；未验证启动后的状态，不能据此判断真实界面已恢复。 |
| `verified` | 后端已核对目标身份和恢复状态匹配。仍不等同于人工验证所有桌面交互。 |
| `pending` | 项目状态已准备，但尚未确认 Codex 已加载。等待应用启动完成后检查项目列表，或重新预览；不显示普通成功提示，也不据此进入中断恢复。 |
| `noOp` | 已是当前账号，无需迁移本地项目。 |

## 中断恢复

如果启动或操作时发现未完成事务，界面会进入恢复页。普通账号操作、设置保存和刷新请求暂停，账号列表和初始化引导不会遮挡恢复入口，也不会把默认设置作为已加载设置保存。

`PROJECT_SHARING_RECOVERY_REQUIRED` 表示存在需要恢复的未完成事务。`pending` / `PROJECT_SHARING_RESTORE_PENDING` 只表示启动后的状态尚未核对成功；事务日志可能已清理，不能将它当作中断恢复要求。相同 pending warning 不会重复提示，启动失败等独立 warning 仍会显示。

点击“恢复未完成切换”后仍需明确确认。确认会先关闭 Codex，再调用恢复命令。取消不执行恢复。成功后重新读取设置和账号，提示可以自行重新打开 Codex 或重试切换；不会声称 Codex 已自动启动。若后台没有待恢复事务，也会重读状态并明确说明。

检测到外部修改或恢复依据冲突时，恢复会拒绝覆盖。界面保留恢复入口与本地化原因，不显示原始路径、凭据或后台错误文本。不要手工删除事务文件来绕过冲突检查；先保留现有文件并排查冲突。

## CLI

以下使用本分支构建的 CLI；已发布的 `ctc` 安装包不一定包含这些命令。`<id>` 是目标账号标识，`<previewFingerprint>` 来自紧邻本次操作的只读预览。

```bash
./src-tauri/target/debug/codex-tools-cli switch <id> --preserve-local-projects --dry-run --json

./src-tauri/target/debug/codex-tools-cli switch <id> --preserve-local-projects --preview-fingerprint '<previewFingerprint>' --accept-project-merge --json

./src-tauri/target/debug/codex-tools-cli projects recover --json
```

`--dry-run` 不刷新授权、不停止桌面进程、不写源数据。它输出预览计数、`requiresMergeConfirmation` 和 `previewFingerprint`。需要合并时，`--accept-project-merge` 必须与当前 `--preview-fingerprint` 配对。没有合并确认要求时可省略 `--accept-project-merge`，仍应提供指纹。需要自动启动时另加 `--launch`；不加仍会先关闭 Codex。

持久设置 `preserveLocalProjects=true` 同样作用于 CLI/TUI。指定 `--codex-home <目录>` 的普通切换不受支持；须走保留流程。指定 home 的 TUI 要求启用保留，且拒绝 `--refresh`，避免隐式刷新到错误目录。使用默认 home 的原有刷新行为保持不变。托盘等非交互入口不能绕过需要的预览与合并确认，应返回桌面界面或使用带当前指纹的 CLI 流程。

隔离使用时同时指定 `--data-dir <账号数据目录>` 和 `--codex-home <Codex数据目录>`。`projects recover` 使用相同目录参数恢复对应事务；它不是只读命令，会停止 Codex 并尝试恢复。

## 支持范围与限制

- 支持本地 ChatGPT 的账号分区状态 v1，结构依据已观察的桌面构建 25241、25894。对未知格式、身份不一致、异常 provider 路由、数据库不匹配或云端绑定等情况拒绝修改。
- 保留项目定义、项目与会话归属，以及受支持的本地项目展示元数据；会话正文、云端权限和账号配额不因此跨账号同步。远端或云项目不能按本地项目迁移。
- 从未管理、且指向缺失项目的旧关联、项目排序和服务端映射原样留在所属账号，不阻断无关项目。旧聊天排序中已无有效归属的记录也不会阻断切换。若旧项目元数据与共享范围发生碰撞则拒绝操作；历史归属以各账号自己的基线判断。SQLite 暂无某线程行不会被当作删除归属的依据，仍保留有效项目关系及明确未分组记录；这不等于恢复会话正文。
- 原生生命周期支持 macOS/Windows；Linux 不支持此保留切换的桌面生命周期。
- 切换和恢复使用合成数据与 mock IPC 验证；原生 Windows 生命周期与 ACL 尚未进行实机验收。测试不使用真实账号、令牌或项目数据。

## 可重复合成验证

前端检查不启动真实桌面：

```bash
npm test
npm run build
npm run lint
npm run test:pages
```

CLI 冒烟验证使用临时目录中的 24 个虚构项目、81 条项目会话关联和 4 条未分组会话，并检查错误路由拒绝、源文件与目录 hash 不变。先构建指定目标，再通过隔离脚本运行：

```bash
bash scripts/test-isolated.sh cargo build --manifest-path src-tauri/Cargo.toml --bin codex-tools-cli --target-dir /private/tmp/codex-tools-target
bash scripts/test-isolated.sh python3 scripts/check-project-sharing-cli.py --cli /private/tmp/codex-tools-target/debug/codex-tools-cli
```

浏览器只使用 `tests/ui/preview.html` 的合成后端，可用 `npm run dev -- --host 127.0.0.1` 启动预览：

| 查询参数 | 合成场景 |
| --- | --- |
| 无 | 默认开启；切换调用项目预览。 |
| `?preserve=0` | 显式关闭；切换不调用项目预览。 |
| `?preserve=1` | 24 项目 / 81 关联 / 4 未分组；目标已有 3 个项目，须确认。 |
| `?preserve=1&emptyTarget=1` | 无需合并确认，仍传指纹。 |
| `?preserve=1&switchError=PREVIEW_CHANGED` | 确认后拒绝过期预览。 |
| `?preserve=1&preservationStatus=pending` | 项目状态已准备但启动后未确认加载，显示检查项目列表或重新预览的提示。 |
| `?pendingRecovery=1` | 启动即发现未完成事务，显示恢复入口。 |
| `?pendingRecovery=1&recoveryError=externalChange` | 恢复因外部修改拒绝覆盖。 |

可附加 `&locale=en-US`、`ja-JP`、`ko-KR`、`ru-RU` 检查其他语言。mock 暴露 `window.__previewCommands` 和 `window.__previewCalls`，用于确认取消没有切换或恢复调用、指纹参数正确且恢复后重读了设置和账号。这些是合成验证入口，不能作为线上或真实桌面验收结果。
