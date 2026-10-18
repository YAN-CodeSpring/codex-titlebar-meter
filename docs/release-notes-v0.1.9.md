## 🎨 外观、字体与双模块布局

- 背景新增“液态玻璃（实验）”：半透明质感与边缘反光，响应标题栏颜色和鼠标位置；可切回实色或透明。
- 菜单新增“形状 → 圆角 / 方角”。默认圆角，两块额度面板各自拥有完整轮廓，中间留 6 个逻辑像素的透明间隔；单额度只显示一块面板。
- 形状、主题、背景和强调色分别设置，选择立即生效并保存，升级保留原有设置。
- 中文默认微软雅黑 UI，英文 Segoe UI；3 倍分辨率绘制后缩采样，改善小字号中文和数字的笔画粗细与边缘。
- 减少重复设置窗口属性和位置，缓存相同画面，实色模式加入离屏缓冲，降低闪动。
- 保留七种强调色、低额度警示、最大化／全屏支持和原有登录自启动逻辑。

35 项自动化测试通过，包括 100%、125%、150%、200% 缩放下两个面板的形状一致性、透明间隙、文字合成和资源释放。实际字体效果已获用户确认，双圆角布局已在本机检查。见[绘制说明](https://github.com/YAN-CodeSpring/codex-titlebar-meter/blob/v0.1.9/docs/rendering.md)。

## ⬇️ 下载与安装

下载下方 `codex-titlebar-meter-v0.1.9-windows-x64.zip` 并完整解压，直接运行 exe 可使用便携版。

需要升级固定安装、保留设置并在 Windows 登录后自动启动时，在解压目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装版在 Windows 登录后驻留后台，打开或重新打开 Codex 后自动显示。若手动结束用量条进程，仅打开 Codex 不会再次启动它。便携版不会注册登录自启动。未实际重启电脑测试。

本版本仍不单独区分未登录、API 登录或订阅到期；读取失败使用重试提示，不会把失败判定为订阅过期。CLI 缓存沿用上游的旧版本清理机制。

## English

Adds experimental translucent glass material and persistent Rounded / Square settings. Dual quotas appear as separate cards with a DPI-scaled 6 logical pixel gap. Text is supersampled for smoother small Chinese and Latin glyphs; unchanged frames and window positions are no longer repeatedly submitted, and solid rendering is buffered. All 35 automated tests passed. Existing themes, accents, fullscreen support, login startup, and old CLI cache cleanup are retained.

Download the Windows x64 ZIP, extract it, and run the executable for portable use, or run `install.ps1` for an upgrade with per-user login startup. Preferences are preserved. Account-state-specific messages remain planned, not implemented in this release.

Windows x64. Unsigned executable. Original project: [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter), MIT; fork maintained by YAN-CodeSpring. Original attribution, artwork, and license are preserved.
