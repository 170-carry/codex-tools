# Git 统计版：GitHub Actions 打包 Mac

在自己的 GitHub 仓库使用 `.github/workflows/git-activity-macos.yml`。
该流程只生成下载附件，不创建 Release、不发布 npm，也不需要原作者的签名密钥。

## 使用

1. 将应用源码和此工作流提交到有写入权限的仓库默认分支；公开源码前确认不包含个人或公司数据。
2. 打开仓库 **Actions → Build Git Calendar for macOS → Run workflow**。
3. 等待两项构建完成，在这次运行底部的 **Artifacts** 下载：
   - `CodexTools-Git-macOS-AppleSilicon`：M 系列芯片。
   - `CodexTools-Git-macOS-Intel`：Intel 芯片。
4. 解压下载附件，打开其中的 `.dmg`，将应用拖入 Applications。

附件保留 14 天，包含 DMG、应用 ZIP 和 SHA256 校验清单。
更新源码后重新运行该流程即可得到新版安装包。
不要使用原项目的 `Release Tauri App` 流程来发布这个定制版本。

## 数据与配置

只上传应用源码，不上传公司仓库、统计报告、员工配置、令牌或本地缓存。
`release/`、`.codex-verification/`、`src-tauri/gen/`、`node_modules/` 等已由 Git 忽略。
安装后在 Mac 的仓库设置中选择本地目录或填写有访问权限的远程 Git 地址。
运行统计需要 Mac 已安装 Git，远程私有仓库还需配置相应的 Git 访问凭据。

构建依次合并基础配置、`tauri.git-activity.conf.json` 和
`tauri.git-activity.macos.conf.json`；最后一项覆盖 Windows 的打包类型。
原有 Windows 打包配置不受影响，定制版禁用原作者的更新下载地址。

## 签名与验证

目前使用 ad-hoc 签名，不包含 Apple 开发者证书或公证。
首次运行可能需要在 macOS“系统设置 → 隐私与安全性”中允许打开。
正式对外分发时，再配置 Developer ID 签名和 Apple 公证。

云端会执行代码检查、现有前端测试、Git 采集器测试和前端构建，
再检查安装包中可执行文件的架构及签名。
构建成功不等于 Mac 上的完整交互验收；首次安装后仍需验证目录选择、
远程仓库读取、人员合并/排除、自动刷新和 HTML 导出。

参考：[Tauri GitHub Actions](https://v2.tauri.app/distribute/pipelines/github/)、
[macOS 签名](https://v2.tauri.app/distribute/sign/macos/)。
