# Issue #229：保留 ChatGPT 登录态接入本机反代的可行性验证

验证日期：2026-10-16。关联问题：<https://github.com/170-carry/codex-tools/issues/229>。

## 结论与边界

本机协议验证通过：ChatGPT 登录身份可以与本机反代的鉴权、上游账号分开。
本次使用已安装的 Codex CLI/app-server 和 Codex Tools proxyd，真实调用账号池上游。
没有修改正在使用的 `~/.codex/auth.json`、`~/.codex/config.toml` 或正式账号库。

真实手机 Remote 尚未验收。用户暂时不方便操作手机，因此未验证手机设备配对、
云端连接、手机 UI 新建任务以及手机重连。下文双客户端测试验证的是本机共享
app-server 的协议能力，不能替代手机端到端验收。

## 环境

- Codex/ChatGPT 桌面：26.1007.71326，build 20398。
- 内置 Codex CLI：0.162.0-alpha.17.2。
- Codex Tools / proxyd：3.1.1。
- 项目基线：`b5cb995f13f92b13e1c72d025270df0c150f5a45`。
- 测试模型：`gpt-6.1-sol`，low reasoning，default service tier。
- 使用单独的临时 CODEX_HOME、账号库、代理 Key、loopback 端口及测试工作目录。
- 账号池仅放一个与 ChatGPT 登录身份不同、访问令牌尚未过期的账号。
- 代理请求观察器只记录状态、字节数、摘要和鉴权匹配布尔值，不记录凭据或正文。

## 实测结果

| 检查 | 结果 |
| --- | --- |
| 正常模式新建和回复 | completed，返回 NORMAL229 |
| 反代模式 account/read | type=chatgpt，requiresOpenaiAuth=true |
| 反代模式新建和回复 | completed，返回 POOL229 |
| 同时携带 ChatGPT Authorization 和独立 x-api-key | 4 个实测代理请求均匹配，HTTP 200 |
| 反代会话读取 | thread/read 成功，能够读取已完成 turn |
| 重启 app-server 后继续反代会话 | completed，正确记住 POOL229 |
| 第二个独立 app-server 读取会话 | 成功 |
| 第二个独立 app-server 接管被占用会话 | 拒绝：already has an active writer |
| 两个客户端连接同一个 app-server | 第二客户端可以读取 inProgress turn，并成功 resume |
| 共享服务上的实时进度 | 第二客户端收到 42 个 agentMessage delta |
| 第二客户端继续会话 | completed，正确记住 POOL229 |
| 恢复 openai 后继续原正常会话 | completed，正确记住 NORMAL229 |
| 显式将测试池会话切回 openai 后继续 | completed，正确记住 POOL229 |
| 正式 auth/config 文件 | 测试前后 SHA-256 一致 |

详细的脱敏结果和请求观测保存在
[validation/issue-229-remote-compatibility.json](validation/issue-229-remote-compatibility.json)。

## 有效的配置方向

保留 ChatGPT 登录文件，新增一个自定义 provider。以下只展示关键字段，并非一键
覆盖现有配置的文件：

```toml
model_provider = "codex_tools_remote"

[model_providers.codex_tools_remote]
name = "Codex Tools Remote"
base_url = "http://127.0.0.1:8787/v1"
wire_api = "responses"
requires_openai_auth = true
supports_websockets = false

[model_providers.codex_tools_remote.http_headers]
x-api-key = "<LOCAL_PROXY_KEY>"
```

`Authorization` 保留 ChatGPT token；本项目已有的 `x-api-key` 优先逻辑识别本地
代理 Key。proxyd 使用其账号池中的上游身份处理推理。Key 必须允许目标模型、
推理强度和服务等级。包含 Key 的配置文件应维持私有权限；不向报告写出 Key。
本次保留了已有静态模型目录，尚未验证无静态目录时的自动模型发现。

## 实现时必须处理的行为

1. **会话列表按 provider 过滤。** 反代模式默认 thread/list 包含池会话但不含正常
   会话；恢复正常模式后则相反。显式 `modelProviders: []` 可以列出两类会话。
   修改真实客户端的 UI 列表行为属于后续兼容性工作。
2. **旧会话有自己的 provider。** 仅改全局配置不会可靠地迁移已有会话；本次
   将池会话切回正常服务时，显式使用了 thread/resume 的 modelProvider=openai。
   同样，切进反代时必须核对 resume 实际返回的 modelProvider 和请求去向。
3. **共享服务与独立进程不同。** 两个客户端共享同一服务可以查看实时进度并继续
   会话；另启独立 app-server 会遇到写入者锁。不能通过删除锁或修改会话数据库
   规避它。本次现象与 issue 描述相似，但未定位提问者手机报错的实际原因。
4. **正常模式恢复不应重写 ChatGPT auth。** 此方案始终不切换登录身份，恢复应
   处理本方案管理的配置及会话 provider，不应把旧 token 备份写回覆盖刷新结果。
5. **loopback 不应绕经系统代理。** 本机原有代理环境使首轮请求在到达观察器前
   返回 502；加入 NO_PROXY/no_proxy 的 127.0.0.1、localhost、::1 后消失。

## 下一步验收

在真实手机可操作时，再验证：官方 Remote 使用同一账号/工作空间连接现有主机；
手机新建测试会话确实命中代理；运行时离开再打开能看到进度；手机继续会话仍走
代理；恢复后可找到并继续原正常会话。记录返回的 provider、实际代理请求和
错误原文。上述全部通过前，不宣称 Remote 完整支持。

本次没有新增产品开关、提交或发布版本，也没有制作小组件。

## 复现脚本

`scripts/remote-compat-probe/` 分别实现隔离环境、JSON-RPC 客户端、脱敏请求
观察、基础验证及共享服务双客户端验证。Python 3.12 和 websockets 13.1 为本次
实际使用版本。运行 run.py 后再运行 shared_server.py；临时目录路径由脚本输出。
脚本会消耗少量真实模型额度。临时目录含测试所需的凭据副本，验收后必须清理；
只保存脱敏报告。不会调用账号库导出命令或执行 SQL。

## 官方参考

- [ChatGPT 登录与网关鉴权分离](https://learn.chatgpt.com/docs/enterprise/sign-in-with-chatgpt-through-a-gateway)
- [配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)
- [Remote connections](https://learn.chatgpt.com/docs/remote-connections)

官方文档说明配置机制；本次账号池替换上游身份的行为以实测结果为依据，不等同于
官方对第三方账号池的兼容性承诺。
