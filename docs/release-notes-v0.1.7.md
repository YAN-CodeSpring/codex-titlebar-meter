## 外观设置 / Appearance settings

- 点击右侧 `...` 或右键用量条打开设置菜单。
- 默认自动跟随 Codex 标题栏明暗，也可指定浅色或深色。
- 可选透明背景，文字、进度条和小型设置按钮保持可见。
- 七种强调色：蓝、绿、紫、青、橙、粉、石墨灰。
- 设置立即生效并保留，升级时沿用已有强调色。
- 保留最大化、全屏显示和关闭 Codex 后的后台监视。

## 下载与自启动

下载下方 `codex-titlebar-meter-v0.1.7-windows-x64.zip`，完整解压后运行 exe 即可使用便携版。

需要 Windows 登录后自动启动时，在解压目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装版会在 Windows 登录后驻留后台，尚未打开 Codex 时等待，打开或重新打开 Codex 后自动显示。便携版需要手动启动；如果手动结束用量条进程，仅打开 Codex 不会重新启动它。升级安装保留外观设置及登录自启动。

发布前核对了当前用户的登录自启动注册、后台监视与可见浮层，并通过模拟所有者窗口关闭与重建的生命周期测试；未实际重启电脑验证。

Click the dots or right-click the meter for appearance, background, and accent settings. Auto follows the visible Codex title bar, with Windows theme fallback; Light and Dark can be selected manually. Choose a solid or transparent background and one of seven accents. Preferences persist across restarts and upgrades.

Auto samples seven title-bar pixels locally without saving screenshots. Custom title-bar artwork may affect detection; select a manual theme when needed. Transparent areas pass clicks through to Codex and text uses non-antialiased rendering to avoid color fringes.

26 automated tests cover layout, window lifecycle, data handling, settings migration, appearance commands, and light/dark foreground selection. Desktop verification confirmed the light theme and opening the settings menu.

Windows x64. This executable is unsigned. Original project: [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter), MIT; fork maintained by YAN-CodeSpring.
