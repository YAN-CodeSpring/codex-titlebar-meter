# Codex Titlebar Meter v0.1.5 — 全屏显示改进

这是 [YAN-CodeSpring/codex-titlebar-meter](https://github.com/YAN-CodeSpring/codex-titlebar-meter) 的首个发布版，基于 [ConfigCrate 原项目](https://github.com/configcrate/codex-titlebar-meter) v0.1.4，保留原作者版权和 MIT 许可证。

## 改进

- 修复最大化时的窗口边框定位偏移。
- 无边框全屏时在窗口内部顶部右侧持续显示额度。
- 切换到其他应用、最小化或切换虚拟桌面时隐藏用量条。
- 全屏时保留颜色切换，禁用拖动和双击窗口操作。
- 通过 21 项自动化测试；用户已确认新版在实际桌面环境中正常显示。

## 下载与使用

1. 下载下方的 `codex-titlebar-meter-v0.1.5-windows-x64.zip`，完整解压；不要选择 Source code。
2. 退出旧版用量条，双击解压后的 `codex-titlebar-meter.exe`。
3. 打开并登录 Microsoft Store 版 Codex Desktop。无需安装 Rust，也不需要 API Key。
4. 如需开机自启，在解压目录的 PowerShell 中运行 `powershell -ExecutionPolicy Bypass -File .\install.ps1`。安装无需管理员权限。

适用 Windows 10/11 x64。ZIP 包含程序、安装/卸载脚本、README 和原始 LICENSE；请保留许可证。相邻的 `.zip.sha256` 文件提供 ZIP 的 SHA-256 校验值，可用 PowerShell 的 `Get-FileHash` 核对。本程序未进行代码签名。

## English

First release of the YAN-CodeSpring fork, based on ConfigCrate v0.1.4, with the original copyright and MIT license preserved. Adds maximized and borderless fullscreen support, corrects overlay positioning, and hides the meter while another application is active. All 21 tests pass; desktop use has also been confirmed by the user.

Download and fully extract the Windows x64 ZIP below. Exit any older meter and double-click `codex-titlebar-meter.exe`, then open the Microsoft Store version of Codex Desktop and sign in. No Rust installation or API key is needed. Run `install.ps1` from PowerShell if you want automatic startup at Windows sign-in. The ZIP includes the original LICENSE; the adjacent SHA-256 file verifies the archive. The executable is unsigned.
