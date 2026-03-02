# Codex Tools

一个基于 **React + Tauri** 的桌面工具，用来管理多个 Codex 账号：
- 看每个账号的用量
- 快速切换账号
- 自动拉起 Codex

仓库地址：<https://github.com/170-carry/codex-tools>

## 更新日志
- v0.3.2
  1. 增加用量排序
- v0.3.1
  1. 优化UI表现
- v0.3.0
  1. 修复关闭无效的问题
  2. 优化弹窗样式
- v0.2.7
  1. 增加 用量接口候选
  2. 修复 添加账号偶尔出现消失问题
- v0.2.5
  1. 优化 app 启动退出。
- v0.2.4
  1. 优化整体 UI 布局。
  2. 增加后台运行。
- v0.2.3
  1. 设置增加切换账号是否重启编辑器（兼容 Codex 编辑器插件）。
  2. 修复启动稳定性问题（配置文件报错时自动恢复，避免崩溃）。
  3. 修复 Windows 下 opencode 认证文件目录识别不正确的问题。
- v0.2.2
  1. 设置中增加 Opencode 快捷切换功能，开启后切换 Codex 同时会切换 Opencode 授权。
  2. 优化下载页样式。
- v0.2.1：优化整体启动方式。

## 应用截图

![Codex Tools Screenshot](public/ScreenShot.png)

## 解决codex-tools app 已损坏的方案

> https://zhuanlan.zhihu.com/p/135948430

> 省流:

> sudo spctl  --master-disable

> sudo xattr -r -d com.apple.quarantine /Applications/Codex\ Tools.app

## 快速启动（本地开发）

### 1) 环境准备

- Node.js 20+
- Rust stable
- macOS 或 Windows（优先支持 macOS）

### 2) 安装依赖

```bash
npm install
```

### 3) 启动桌面应用

```bash
npm run tauri dev
```

就这三步。

## 主要功能

### 1. 多账号管理

- 添加账号：点击「添加账号」后打开授权流程
- 授权成功后自动回收并加入列表
- 删除账号：支持一键删除账号

### 2. 用量监控

- 显示每个账号的 **5h** 和 **1week** 窗口
- 显示 **已用百分比 + 剩余百分比**
- 每 30 秒自动刷新，也可手动刷新

### 3. 一键切换账号

- 点「切换并启动」即可切到目标账号
- 后台静默探测 Codex App 并启动
- 找不到 App 时自动回退到 `codex app`

### 4. 添加账号不影响当前账号

- 添加新账号时会先备份当前登录状态
- 添加结束后自动恢复，避免当前账号被替换

### 5. 计划识别与视觉区分

- 自动识别 Free / Plus / Pro / Team 等计划
- 不同计划有不同卡片边框风格
- 当前账号会高亮显示

### 6. 应用更新

- 启动时可检查 GitHub Releases 新版本
- 支持在应用内下载更新并重启

## 打包与发布（简版）

本项目已配置 GitHub Actions 自动发布（mac 双架构 + Windows）。

触发发布：

```bash
git tag v0.1.3
git push origin v0.1.3
```

查看：
- 代码仓库: <https://github.com/170-carry/codex-tools>
- 版本发布: <https://github.com/170-carry/codex-tools/releases>

## 目录说明

- 前端：`src/`
- Tauri / Rust：`src-tauri/`
- 发布流程：`.github/workflows/release.yml`

## License

MIT，详见 [LICENSE](LICENSE)。
