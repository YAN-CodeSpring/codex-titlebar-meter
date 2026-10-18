# v0.1.10 — 修复关闭 Codex 后用量条消失

从 Codex 内运行旧安装脚本时，用量条会继承安装终端的 Windows Job。宿主退出时，该 Job 可以连带终止用量条；仅重建浮层窗口无法解决整个进程被结束的问题。

- 安装器通过已有的 Windows 桌面 Explorer 独立启动用量条，避免继承安装终端的 Job。没有新增常驻辅助进程或服务。
- 重建浮层时清除实色画面缓存，确保新窗口重新绘制。
- 升级保留外观设置和当前用户登录自启动。已有安装需要重新运行新版 `install.ps1`，仅替换 exe 不会改变正在运行的旧进程所属 Job。

## ⬇️ 安装或升级

下载下方 `codex-titlebar-meter-v0.1.10-windows-x64.zip`，完整解压后，在解压目录打开 PowerShell 并运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装器会替换旧版、保留外观设置、保留当前用户登录自启动，并立即独立启动用量条。无需管理员权限。便携版也可从文件资源管理器双击 exe 启动，但不会注册登录自启动。

## 验证

验证：35 项 Rust 测试通过。新增 Windows 集成测试实际创建并终止安装终端的 Job：旧方式启动的用量条被结束，修复方式启动的用量条继续存活。测试不关闭正在使用的 Codex；完整退出并重新打开真实 Codex、重启 Windows 仍需实际使用验证。

运行集成测试（先退出已有用量条，测试会启动并清理自己的实例）：

```powershell
python scripts/test-independent-start.py target/package-build/release/codex-titlebar-meter.exe
```

The installer now asks the existing Windows desktop shell to launch the meter independently. This prevents Codex's installation-terminal job objects from terminating the resident meter when Codex exits. No extra helper or service stays running. Run the updated installer to apply the fix; preferences and per-user login startup are preserved. All 35 Rust tests and the real job-termination regression test passed. A full real Codex restart and Windows reboot were not performed during the test.
