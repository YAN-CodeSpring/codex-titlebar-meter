## 外观设置 / Appearance settings

- 点击右侧 `...` 或右键用量条打开设置菜单。
- 默认自动跟随 Codex 标题栏明暗，也可指定浅色或深色。
- 可选透明背景，文字、进度条和小型设置按钮保持可见。
- 七种强调色：蓝、绿、紫、青、橙、粉、石墨灰。
- 设置立即生效并保留，升级时沿用已有强调色。
- 保留最大化、全屏显示和关闭 Codex 后的后台监视。

Click the dots or right-click the meter for appearance, background, and accent settings. Auto follows the visible Codex title bar, with Windows theme fallback; Light and Dark can be selected manually. Choose a solid or transparent background and one of seven accents. Preferences persist across restarts and upgrades.

Auto samples seven title-bar pixels locally without saving screenshots. Custom title-bar artwork may affect detection; select a manual theme when needed. Transparent areas pass clicks through to Codex and text uses non-antialiased rendering to avoid color fringes.

26 automated tests cover layout, window lifecycle, data handling, settings migration, appearance commands, and light/dark foreground selection. Desktop verification confirmed the light theme and opening the settings menu.

Windows x64. This executable is unsigned. Original project: [ConfigCrate/codex-titlebar-meter](https://github.com/configcrate/codex-titlebar-meter), MIT; fork maintained by YAN-CodeSpring.
