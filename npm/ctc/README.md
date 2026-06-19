# @170-carry/ctc

`ctc` 是 Codex Tools 的 npm 命令入口。

安装：

```bash
npm i -g @170-carry/ctc
```

使用：

```bash
ctc list --json
ctc switch 1
ctc switch --best --launch
ctc login --label work
ctc import ./auth.json
ctc export ./accounts.json
ctc usage --cached --json
ctc doctor --json
ctc report --json
ctc tui
ctc ui
```

说明：

- `ctc list/switch/login/import/export/usage/doctor/report/tui` 会调用随 npm 安装的原生 `codex-tools-cli`。
- `ctc ui` 会打开本机已安装的 Codex Tools 桌面应用；如果没有安装桌面应用，请先从 GitHub Releases 安装。
- 如果安装时禁用了 optional dependencies，请重新安装：`npm i -g @170-carry/ctc --include=optional`。
