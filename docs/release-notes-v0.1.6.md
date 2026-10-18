# Codex Titlebar Meter v0.1.6 — 关闭重开后自动恢复

本版本修复关闭 Codex 后用量条一同退出、再次打开 Codex 时不再显示的问题。此 Fork 基于 [ConfigCrate 原项目](https://github.com/configcrate/codex-titlebar-meter)，保留原作者版权与 MIT 许可证。

- 增加独立的后台监视窗口，Codex 关闭时不会终止监视进程。
- Codex 重新打开或更换主窗口时，自动重建并附着额度浮层。
- 保留最大化、无边框全屏、颜色设置和数据刷新逻辑。
- 通过 22 项测试，包括实际 Win32 主窗口关闭与浮层重建的两轮回归测试。

## 下载与安装

下载 `codex-titlebar-meter-v0.1.6-windows-x64.zip` 并完整解压。退出旧版用量条后，在解压目录打开 PowerShell 并运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装版会在当前用户登录 Windows 时自启，之后等待 Codex 打开；不需要管理员权限。仅双击 exe 属于便携模式，不会注册自启，电脑重启后仍需手动启动。未打开 Codex 时没有可见浮层。

适用 Windows 10/11 x64、Microsoft Store 版 Codex Desktop。ZIP 保留 LICENSE，并附有 SHA-256 校验文件。本程序未进行代码签名。

## English

Fixes the meter exiting when Codex closes. A separate message-only watcher now survives owner-window destruction and recreates the quota overlay when Codex reopens. All 22 tests pass, including two native Win32 owner-close and overlay-recovery cycles.

Extract the Windows x64 ZIP, exit the old meter, and run `install.ps1` from PowerShell to enable startup at Windows sign-in. Launching only the exe is portable mode and does not enable login startup. The watcher waits invisibly while Codex is closed. Original ConfigCrate copyright and MIT license are preserved; the executable is unsigned.
