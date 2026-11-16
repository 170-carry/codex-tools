# 任务栏嵌入版

本项目是 `codex-tools` 的独立源码副本，保留设置中的水平、垂直偏移和自动保存。运行前请从托盘退出旧版；本副本仍使用原应用标识和数据目录。

本版参考 [TrafficMonitor 的 Win11TaskbarDlg](https://github.com/zhongyang219/TrafficMonitor/blob/930f17533d6098989ebad62f210aa97d75ef174b/TrafficMonitor/Win11TaskbarDlg.cpp) 和 [TaskBarDlg::OnInitDialog](https://github.com/zhongyang219/TrafficMonitor/blob/930f17533d6098989ebad62f210aa97d75ef174b/TrafficMonitor/TaskBarDlg.cpp#L994-L1017) 的原生任务栏嵌入机制，在 Rust 中重新实现，未复制其 C++ 源码。

左右任务栏位置使用 `SetParent` 挂到主任务栏 `Shell_TrayWnd`，切换为 `WS_CHILD`，在任务栏内部使用 `HWND_TOP`。任务栏抬高时，小组件随父窗口一起变化，不再通过定时器抢回桌面置顶层级。保留的 100 毫秒检查用于全屏/自动隐藏状态；只有原有的悬浮回退模式才使用桌面层级修复。

屏幕坐标加上 DPI 缩放后的偏移，再转换为父窗口客户区坐标。偏移数值仍自动保存；移出任务栏的部分会被系统裁剪。原先因空间不足或竖向任务栏触发的悬浮回退仍保留，该模式不具备嵌入方式的层级关系。

窗口线程在创建前匹配任务栏的 DPI 感知，清单声明 Windows 10/11 兼容以支持分层子窗口透明绘制。点击恢复主界面通过主线程异步执行，避免在与 Explorer 关联的窗口线程中等待 Tauri 窗口操作。Explorer 重建时沿用原有窗口重建流程。实现没有修改 Explorer 的窗口样式、图标布局或系统设置。

2026-11-16 验证：

- 25 项 Windows 小组件测试通过，包括真实 User32 窗口的嵌入/脱离、父窗口反复抬高和移动、坐标转换、不抢焦点、隐藏/恢复与父窗口销毁；保留偏移、文字渲染和全屏判断回归测试。
- 隔离测试版使用空账号配置，在本机 144 DPI 环境确认 `GetParent(widget) == Shell_TrayWnd`、`WS_CHILD`、透明表面更新成功。
- 实际打开/关闭开始菜单，连续 100 秒采集 6,406 次窗口状态，未出现隐藏或脱离；采样期间图标位置命中小组件。
- 这属于窗口状态采样，并非显示器逐帧录像。没有实机复测全屏切换、Explorer 重启或所有任务栏图标点击，不能据此宣称所有系统情形均已证明“零帧遮挡”。

构建：

```powershell
npm ci
npm run tauri -- build --bundles nsis --config tauri.local-build.json --no-sign -- --locked
```

测试：

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --lib windows_taskbar_widget --locked
```

Windows API 依据：[SetParent](https://learn.microsoft.com/windows/win32/api/winuser/nf-winuser-setparent)、[分层子窗口清单要求](https://learn.microsoft.com/windows/win32/winmsg/using-windows#using-layered-windows)。
