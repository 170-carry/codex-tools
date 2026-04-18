## v1.7.8

基于上游 `v1.7.7`（截至 2026-04-18 的 `upstream/main`）补充以下修复：

1. 修复 Windows 下微软商店版 Codex 启动误命中 `Codex Tools` 自身 AUMID 的问题，改为更严格识别 `OpenAI.Codex` 目标。
2. 微软商店版改为优先通过 AUMID 调用系统激活接口，并在返回前校验 Codex 进程已真正拉起；当应用路径启动失败时，再回退到 `codex app`。
3. 自动清理设置中残留的版本化 `WindowsApps\\OpenAI.Codex_<version>...` 路径；即使本地拿不到稳定 exe 路径，也会优先按 AUMID 直启，而不是误提示缺少 Codex CLI。
4. 新增“重新授权账号”入口，可用新的 OAuth 登录态原位替换旧 `authJson`，无需先删除账号再重登。
5. 重新授权时会校验新旧账号身份一致，并同步更新 `accountId`、`planType` 与 usage 快照，修复旧账号长期显示 `free` 的问题。
6. 未选择任何编辑器重启目标时不再额外报错，避免干扰切号后的启动提示。
