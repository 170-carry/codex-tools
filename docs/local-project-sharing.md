# 本地会话覆盖与恢复

`preserveLocalProjects` 默认开启；已经保存的关闭设置保持关闭。开启时，本地 ChatGPT 账号切换使用当前账号的最终会话快照覆盖目标账号，不再合并两边的项目或逐项验证历史项目结构。

1. 请求 Codex 保存并退出，确认桌面进程已经结束后重新读取最终状态。
2. 保存当前账号的项目、会话归属、分组、置顶、草稿和其他账号会话状态。
3. 将目标账号原来的会话状态保存为备份，再用当前快照覆盖。目标账号独有的本地项目和分组不会合并进来。
4. 写入目标账号自己的登录信息，并按设置重新启动 Codex。

来源快照保存在账号数据目录的 `profiles/<来源账号 id>/desktop-state.json`，目标覆盖前的快照保存在 `profiles/<目标账号 id>/desktop-state.previous.json`。每个账号保留最近一次对应快照，不持续累积完整数据库副本。登录凭据仍使用目标账号自己的 `auth.json`。

状态文件中的 `owner: null`、缺失的旧版账号分区、历史残留关联、重复排序和项目扩展字段都不会阻断复制。目标状态的旧结构和旧合并登记文件也不会阻断覆盖。无法解析的 JSON、登录身份不明或文件读写失败仍会报错。

会话正文、SQLite 索引和工作目录继续使用同一个本地 Codex 数据目录，不重复复制或重写数据库。账号分区中的会话引用原样复制；服务端内容的访问权限仍由所登录的账号决定。

## 事务与恢复

切换在同一事务中保存来源快照、目标备份、账号配置和全局状态。正式写入前检查文件是否在 Codex 退出后又被修改；真实的并发写入仍会中断操作，避免覆盖另一个正在运行的写入者。

`prepared` 表示已写入，`verified` 表示启动后核对通过，`pending` 表示已写入但尚未确认客户端加载完成。`pending` 不等同于需要回滚。

未完成事务会显示恢复入口并暂停普通账号操作。点击恢复并确认后，会先关闭 Codex，再按事务记录恢复；不会自动重新启动 Codex。外部文件与恢复记录不一致时不会覆盖，不要手工删除事务文件绕过恢复。

## CLI

```bash
./src-tauri/target/debug/codex-tools-cli switch <id> --preserve-local-projects --dry-run --json
./src-tauri/target/debug/codex-tools-cli switch <id> --preserve-local-projects --launch --json
./src-tauri/target/debug/codex-tools-cli projects recover --json
```

只读预览不停止客户端、不刷新授权、不写源文件。切换不再要求合并确认；保留旧的 `--accept-project-merge` 参数以兼容已有调用。预览指纹只约束账号和路由，退出时新增的本地项目、草稿或会话状态会按最终快照保存和复制。

可同时指定 `--data-dir <账号数据目录>` 与 `--codex-home <Codex数据目录>`。恢复时使用同一组目录参数。原生桌面生命周期支持 macOS 和 Windows。

## 验证

回归测试使用独立临时目录中的合成账号、项目和会话。覆盖空账号、旧状态、空 owner、目标覆盖与备份、退出时的最后写入、凭据刷新、事务回滚和中断恢复。数据库和会话正文文件通过字节比较确认未被改写。

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib --no-run
npm test
npm run build
```

macOS 测试二进制通过 `scripts/test-isolated.sh` 运行，防止测试访问真实账号和数据。
