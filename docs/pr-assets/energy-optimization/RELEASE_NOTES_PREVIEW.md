# Next release notes preview / 下一版本更新日志预览

> Preview only: `vNext` is a placeholder and does not select the final release version.
>
> 仅供预览：`vNext` 是占位符，不代表已经确定正式版本号。

## English

1. Improve the macOS status bar and usage labels: use the color app icon, default to showing one-week remaining usage, optionally show 5h / 1w labels, hide the entire status item when disabled, and keep the account meters labeled 5h / 1w even when both values are currently identical.
2. Significantly reduce background energy use: remove Codex inference keepalive calls, delay and deduplicate refreshes, pause foreground polling while the window is hidden, and reuse unchanged token-log results.
3. Speed up first-launch loading: show cached accounts immediately and run non-critical startup work concurrently.
4. Fix the macOS status bar after a Plus-to-Pro upgrade: correctly identify the current account and display its usage.

## 中文

1. 优化 macOS 状态栏和用量标签：使用彩色应用图标，默认仅显示一周剩余用量，可选显示 5h / 1w 标签，选择“不显示”时隐藏整个状态项；即使两个周期当前数值相同，账号用量栏仍分别标注 5h / 1w。
2. 大幅降低后台耗电：移除 Codex 推理保活请求，延后并去重刷新任务，在窗口隐藏时暂停前台轮询，并复用未变化的 Token 日志结果。
3. 加快首次启动加载：优先显示本地缓存账号，并发执行非关键启动任务。
4. 修复 Plus 升级为 PRO 后状态栏无法正确识别当前账号并显示用量的问题。

## In-app appearance / 应用内效果

![Localized in-app release notes preview](./in-app-release-notes-live-preview.jpg)

## Publication mapping / 发布映射

- GitHub Release body: both language sections above.
- Tauri updater `latest.json.notes`: the same bilingual body.
- In-app dialog: selects Chinese for a Chinese UI and English for other locales, with fallback when only one language is available.
- Missing optional notes: emit a build warning, use generated generic release text, and continue publishing.

- GitHub Release 正文：包含以上中英文两部分。
- Tauri updater 的 `latest.json.notes`：写入同一份双语正文。
- 应用内弹窗：中文界面选择中文，其他语言界面选择英文；只有一种语言时自动回退显示可用内容。
- 缺少可选更新说明：输出构建警告，使用自动生成的通用发布文字，并继续发布。

## Missing-notes fallback / 缺少说明时的回退

If a release tag has neither a matching version entry nor reusable `Unreleased` content, the workflow emits a warning and continues with a generated bilingual version-and-platform summary. The existing `v2.4.0` release below shows the repository's historical generic structure; the new fallback keeps that structure and adds both language sections.

如果发布标签既没有匹配的版本条目，也没有可复用的 `Unreleased` 内容，工作流会输出警告，并使用自动生成的双语版本与构建平台摘要继续发布。下面现有的 `v2.4.0` Release 展示了仓库原有的通用结构；新的回退会保留这一结构并补充中英文两个部分。

![GitHub Release without custom notes](./github-release-no-custom-notes.png)

The in-app updater selects the matching language section from the same generated body. In a Chinese UI it therefore shows the concise Chinese version-and-platform fallback instead of an empty changelog.

应用内更新器会从同一份自动生成正文中选择当前界面的语言。因此中文界面会显示简洁的中文版本与构建平台回退内容，而不是空白更新日志。

![In-app updater without changelog notes](./in-app-no-changelog-fallback.png)
