# Codex Titlebar Meter — Codex 额度监控与全屏显示

在 Windows 上实时查看 **OpenAI Codex Desktop 剩余额度、使用限制和重置时间**。用量条显示在标题栏右侧，支持最大化与无边框全屏，无需打开账户菜单，也不需要 API Key。

A Windows **Codex quota monitor and usage meter** for OpenAI Codex Desktop. See remaining quota, rate limits, and reset times in a title bar overlay, with maximized and borderless fullscreen support.

[下载 Windows x64 程序 / Download](https://github.com/YAN-CodeSpring/codex-titlebar-meter/releases/latest) · [安装与使用](#安装) · [English](#english)

> 本仓库是 [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter) 的 Fork，由 [YAN-CodeSpring](https://github.com/YAN-CodeSpring) 维护此修改版。原项目由 ConfigCrate 开发，保留原始 MIT 许可证及版权声明。
>
> 本 Fork 的改进：支持最大化和无边框全屏时显示额度，修正最大化边框偏移，并在切换到其他应用时隐藏用量条。v0.1.6 起关闭 Codex 后保持后台监视，重新打开时自动恢复显示。v0.1.7 增加自动深浅配色、透明背景和七种强调色。已通过 26 项自动化测试，且用户已确认全屏显示在实际桌面环境中正常工作。
>
> This fork of [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter) is maintained by [YAN-CodeSpring](https://github.com/YAN-CodeSpring). It adds maximized and borderless fullscreen support, corrects positioning, and hides the overlay when another app is active. Original copyright and MIT license are preserved. The original project documentation and artwork follow below.

[中文](#中文说明) · [English](#english)

![Codex Titlebar Meter 中文效果图](assets/github-hero-zh.png)

## 中文说明

无需点击账户菜单，直接在 Codex Desktop 标题栏查看剩余额度。

Codex Titlebar Meter 是一个原生 Windows 伴生程序。它把 Codex 返回的真实额度窗口、剩余百分比和重置日期显示在 Codex 标题栏空白区域，并随 Codex 移动、缩放、最大化、最小化和退出。

## 特点

- 零点击：使用 Codex 时始终可见。
- 自动跟随：只附着在 `OpenAI.Codex` 桌面窗口，不覆盖其他应用。
- 最大化与全屏：最大化时校正窗口边框偏移；无边框全屏时常驻窗口内部顶部右侧，切换到其他应用时隐藏。
- 真实数据：通过本机 `codex app-server` 读取，不估算 Token。
- 无需 API Key：不读取、不复制、不保存登录令牌。
- 不修改 Codex：不注入 DLL，不改应用文件，不受普通 Codex 更新覆盖。
- 按实际额度显示：账号没有返回短周期窗口时，不会伪造一个进度条。
- 低额度警示：剩余不超过 20% 时进度条变黄，不超过 10% 时变红。
- 失效可见：连续两分钟没有读到新数据时，进度条变灰并明确提示正在重连。
- 紧凑不挡菜单：单额度窗口仅占 220 个逻辑像素，并固定在标题栏右侧安全区域。
- 跟随 Codex 语言：读取 Codex 的 `localeOverride`，自动显示中文或英文；选择自动检测时跟随系统语言。
- 轻量原生：Rust + Win32/GDI，无 WebView、Electron 或后台 Windows 服务。
- 自动管理缓存：Codex 更新后仅保留当前使用的 CLI 副本，并清理旧版本；仍在运行的副本会留待下次清理。
- 易于移除：按用户安装，无需管理员权限，并出现在 Windows“已安装的应用”中。

## 操作

- 拖动用量条：移动 Codex 窗口。
- 双击用量条：最大化或还原 Codex。
- 点击右侧 `...` 按钮或右键用量条：打开设置菜单。
- 外观：自动（默认）、浅色、深色。浅色为白底深色文字，深色为深底浅色文字。
- 背景：实色（默认）或透明；透明模式保留文字、进度条和小型设置按钮。
- 强调色：蓝、绿、紫、青、橙、粉、石墨灰，共七种。低额度时仍优先显示黄／红色警示。

自动模式每两秒读取可见标题栏的七个像素，以多数明暗判断跟随 Codex；无法读取时回退到 Windows 应用主题。不保存截图、不读取对话内容。自定义主题或标题栏图案可能影响判断，可手动指定浅色或深色。设置立即生效并在重启后保留，升级时保留原来的强调色。

透明区域允许点击穿透到 Codex，拖动和双击请在文字、进度条等可见区域操作。透明模式采用无抗锯齿文字以避免透明边缘出现色边。

无边框全屏时，用量条距顶部 8 个逻辑像素，右侧保留 158 个逻辑像素，并随显示器缩放比例调整。全屏时仍可切换颜色，拖动和双击不会触发窗口移动或最大化；退出全屏后恢复标题栏操作。

![Codex Titlebar Meter 标题栏位置说明](assets/titlebar-detail-zh.png)

颜色设置保存在：

```text
%LOCALAPPDATA%\ConfigCrate\CodexTitlebarMeter\settings.json
```

## 语言支持

- 当前原生支持中文和英文。
- Codex 语言为 `zh-*` 时显示中文，其余语言目前回退为英文。
- 程序每两秒检查一次 Codex 的 `localeOverride`，切换界面语言后无需重启本程序。
- 后续版本计划逐步覆盖 Codex Desktop 支持的全部界面语言。

## 安装

1. 打开本 Fork 的 [最新 Release](https://github.com/YAN-CodeSpring/codex-titlebar-meter/releases/latest)，下载 `codex-titlebar-meter-v0.1.6-windows-x64.zip`（不是 GitHub 自动生成的 Source code）。
2. 将 ZIP 完整解压到一个文件夹。
3. 退出正在运行的旧版用量条，然后双击 `codex-titlebar-meter.exe`，即可使用便携版；无需安装 Rust。
4. 打开已登录的 Microsoft Store 版 Codex Desktop，用量条会自动显示。最大化和无边框全屏均可使用。

如果需要登录 Windows 后自动启动，在解压目录中打开 PowerShell，运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装程序会：

1. 复制程序到 `%LOCALAPPDATA%\Programs\CodexTitlebarMeter`。
2. 注册当前用户登录时启动。
3. 立即开始监视 Codex。
4. 在 Windows“已安装的应用”中注册卸载入口。

不安装也可以直接运行 `codex-titlebar-meter.exe` 作为便携版。

## 自动启动与后台运行

- **便携版**：双击 exe 只启动本次进程，不会注册开机自启。重启电脑后需要手动启动，或运行上面的安装命令。
- **安装版**：`install.ps1` 把程序放到固定目录，并在当前用户登录 Windows 时自动启动；不需要管理员权限。
- **关闭后重开 Codex**：从 v0.1.6 起，用量条的后台监视进程会保留。Codex 关闭时隐藏浮层，重新打开后重新附着，不需要再启动一次用量条。
- **尚未打开 Codex**：程序在后台等待，不会显示独立窗口。直接退出后台进程后，需要手动启动或等下次 Windows 登录；程序没有安装 Windows 服务或修改 Codex 的启动方式。

## 卸载

可以从 Windows“已安装的应用”卸载，或者运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
```

保留颜色设置是默认行为。彻底清除缓存和设置：

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall.ps1 -PurgeSettings
```

## 数据与隐私

程序从 Store 版 Codex 安装目录复制其本地 `codex.exe` 到自己的用户缓存，再以只读沙盒参数启动 `codex app-server`。Codex 自己管理现有登录状态；本程序只接收额度百分比、周期和重置时间。

从 v0.1.4 起，程序启动时会自动删除自己缓存的旧版 `codex.exe`。它只清理符合 Codex 包命名的旧目录和其中的程序文件，不会删除设置文件；正在运行而被 Windows 锁定的副本会在后续启动时重试清理。

没有遥测，没有第三方服务器，也不会向 ConfigCrate 上传数据。

## 当前范围

- Windows 10/11 x64
- OpenAI Codex Desktop（Microsoft Store 版）
- ChatGPT 管理的 Codex 额度窗口
- 浅色与深色 Codex 标题栏，可选透明背景

多窗口、ARM64 和更多界面语言将在实际需求出现后再扩展。

## 开发

需要 Rust stable：

```powershell
cargo test
cargo run
```

构建发布包：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build-release.ps1
```

## English

![Codex Titlebar Meter English preview](assets/github-hero-en.png)

Codex Titlebar Meter shows your real Codex quota directly in the empty area of the Codex Desktop title bar. There is no tray click, dashboard, API key, credential copy, or modification to Codex files.

The compact overlay stays on the right side of the title bar so it does not cover the app menus. It follows the Codex window when you move, resize, maximize, minimize, or close it.

Maximized windows use the visible window area to avoid offsets from invisible resize borders. In borderless fullscreen, the meter stays inside the top-right area, with an 8 logical pixel top margin and 158 logical pixels reserved on the right, scaled for the current monitor. It hides when you switch to another app. Color controls remain available in fullscreen; dragging and double-clicking do not move or maximize the window until fullscreen is exited.

### Features

- Zero-click visibility while you use Codex.
- Real quota windows from the local `codex app-server`; no token estimates.
- Clear copy such as `1 week quota 72% · resets Jul 22`, with a progress bar below it.
- No API key, credential copying, telemetry, third-party server, DLL injection, or Codex file modification.
- Compact native Rust + Win32/GDI executable with no Electron, WebView, or Windows service.
- Automatically removes old cached Codex CLI copies after Codex updates; a copy still in use is retried on a later start.
- Per-user installation, automatic login startup, portable mode, and a clean uninstall entry in Windows Settings.
- If Codex does not return a short-term quota window, the meter does not invent one.
- Low-quota warning colors: amber at 20% remaining and red at 10% remaining.
- A gray stale-data state appears after two minutes without a successful refresh while the meter reconnects.

### Controls

- Drag the meter to move the Codex window.
- Double-click it to maximize or restore Codex.
- Click the dots on the right, or right-click the meter, to open settings.
- Appearance: Auto (default), Light, or Dark.
- Background: Solid (default) or Transparent. Text, progress bars, and the small settings button remain visible.
- Accent: Blue, Green, Purple, Cyan, Orange, Pink, or Graphite. Low-quota warning colors still take priority.

Auto checks seven pixels in the visible Codex title bar every two seconds and follows their majority brightness, falling back to the Windows app theme if sampling fails. It does not save screenshots or read conversations. Custom title-bar artwork may affect detection; use a manual theme if needed. Changes apply immediately and persist across restarts, including the existing accent when upgrading.

Transparent regions pass clicks through to Codex; use visible text or bars for dragging and double-clicking. Transparent mode uses non-antialiased text to avoid colored fringes.

The color preference is stored at:

```text
%LOCALAPPDATA%\ConfigCrate\CodexTitlebarMeter\settings.json
```

### Language support

- Chinese and English are currently built in.
- Codex locales beginning with `zh-` use Chinese; all other locales currently fall back to English.
- The meter checks Codex's `localeOverride` every two seconds, so changing the Codex UI language does not require restarting the meter.
- Future releases can add the full set of languages supported by Codex Desktop.

### Install

Download `codex-titlebar-meter-v0.1.6-windows-x64.zip` from this fork's [latest Release](https://github.com/YAN-CodeSpring/codex-titlebar-meter/releases/latest), not the automatically generated source archives. Extract the whole ZIP, exit any older running meter, and double-click `codex-titlebar-meter.exe`. No Rust installation is required. Open the Microsoft Store version of Codex Desktop and sign in; the meter appears automatically, including in maximized and borderless fullscreen windows.

For automatic startup at Windows sign-in, open PowerShell in the extracted folder and run:

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

The installer copies the app to `%LOCALAPPDATA%\Programs\CodexTitlebarMeter`, starts it immediately, registers per-user login startup, and adds an uninstall entry to Windows Settings. You can also run `codex-titlebar-meter.exe` directly in portable mode.

Portable mode does not register login startup. Since v0.1.6, the background watcher survives closing Codex and recreates the overlay when Codex is reopened. If Codex is not running, the watcher waits with no visible window. To start the watcher again after reboot, use the installer or launch the portable executable manually.

### Uninstall

Use Windows Settings > Installed apps, or run:

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall.ps1
```

To remove cached files and preferences too:

```powershell
powershell -ExecutionPolicy Bypass -File .\uninstall.ps1 -PurgeSettings
```

### Data and privacy

The app copies the local `codex.exe` bundled with the Microsoft Store build of Codex into its own user cache, then launches `codex app-server` with read-only sandbox arguments. Codex keeps control of the existing login state; the meter receives only quota percentages, window durations, and reset times.

Since v0.1.4, the app removes its own obsolete CLI copies on startup. It leaves preferences alone and retries later if Windows has locked a still-running copy.

Nothing is sent to ConfigCrate or any third-party server.

### Current scope

- Windows 10/11 x64
- OpenAI Codex Desktop from the Microsoft Store
- ChatGPT-managed Codex quota windows
- Light and dark Codex title bars, with an optional transparent background

Multi-window support, ARM64, and additional UI languages are candidates for future releases based on demand.

## License

MIT. Independent project; not affiliated with or endorsed by OpenAI.

Original project built by [ConfigCrate](https://configcrate.com/). Fullscreen improvements in this fork maintained by [YAN-CodeSpring](https://github.com/YAN-CodeSpring).
