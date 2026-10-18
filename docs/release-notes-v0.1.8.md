## 降低内存占用 / Lower memory usage

纯显示窗口不再初始化不需要的输入法支持，减少第三方输入法组件加载。仅影响本工具 UI 线程，中文显示和设置菜单正常，Codex 和其他应用的输入法设置不变。

本机约 30 秒采样：

| 驻留内存 | v0.1.7 | v0.1.8 |
| --- | --- | --- |
| 主程序 | 约 59 MiB | 约 13 MiB |
| 包含全部子进程 | 约 122 MiB | 约 75–79 MiB |

总计包含官方 Codex 额度读取进程及控制台宿主。exe 约 719 KiB。数据是本机短时观察，随系统和 Codex 版本变化，不是固定上限；CLI 磁盘缓存另计。[完整检查报告](https://github.com/YAN-CodeSpring/codex-titlebar-meter/blob/v0.1.8/docs/performance.md)。

- 保留自动深浅配色、透明背景和七种强调色。
- 保留最大化、全屏显示与独立后台监视。
- 保留上游的旧版 CLI 缓存清理机制，不声称零缓存。
- 26 项自动化测试通过，实际菜单与中文显示已检查。
- 已核对登录自启动项、后台进程及安装程序与测试构建的哈希一致性；未实际重启电脑测试。

## 下载与使用

下载 `codex-titlebar-meter-v0.1.8-windows-x64.zip`，完整解压。便携版直接运行 exe；升级安装并保留设置、自启动时，在解压目录运行：

```powershell
powershell -ExecutionPolicy Bypass -File .\install.ps1
```

安装版在 Windows 登录后后台等待，打开或重新打开 Codex 后自动显示。如果手动结束用量条进程，仅打开 Codex 不会重新启动它。便携版不注册登录自启动。

The display-only UI thread now opts out of IME initialization, avoiding unnecessary third-party input-method modules. Local sampling measured about 13 MiB for the meter and 75–79 MiB including all children, compared with about 59 MiB and 122 MiB before. Measurements are machine-specific and do not establish long-term limits. Existing appearance options, fullscreen support, login startup, background recovery, and old CLI cache cleanup are retained. All 26 automated tests passed.

Windows x64. Unsigned executable. Original project: [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter), MIT; fork maintained by YAN-CodeSpring. Original attribution and license are preserved.
