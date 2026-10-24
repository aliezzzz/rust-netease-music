# Rust Netease Music

基于 Tauri 的网易云音乐桌面客户端。

Rust / GStreamer 播放核心与 Vue 3 界面。采用左侧资料库、底部播放器和可展开队列，支持浅色、深色与系统主题。

Linux 为现有验收平台。Windows x64 的原生托盘、窗口恢复、运行库打包与 CI 已接入，完整 MSVC 构建及真机验收仍待 Windows 环境执行；构建步骤与限制见 [WINDOWS.md](WINDOWS.md)。

已有候选包的路径、启动方法和校验值见 [RELEASE.md](RELEASE.md)。

## 开发

需要 Rust、Node.js 22.12+、GTK3、WebKitGTK 4.1、GStreamer Play 开发文件，以及 GStreamer 音频插件。

```sh
# Ubuntu / Debian（检查安装脚本后运行）
bash scripts/install-deps-debian.sh
npm ci
npm run tauri dev
```

所有命令均从仓库根目录运行。根目录是 Tauri 应用与播放核心共用的 Cargo workspace；本仓库不再包含旧 GTK4 应用源码或 Meson 构建。
Tauri 的 GTK3 绑定与 GStreamer 0.21 系列共用 GLib Rust 0.18 绑定；这是 Rust 绑定版本，系统仍使用发行版的 GStreamer。

## 播放器交互

- 底部播放器和展开的歌词界面提供首选音质菜单，与设置页同步；下次加载歌曲时生效，不代表当前音频流的实际音质。
- 点击底部封面或歌词按钮打开沉浸式播放页：方形专辑封面、播放控制、封面衍生流动背景与大字号同步歌词。展开 / 收起使用双向上移展开与下移收回过渡，退出动画结束后再恢复底层操作与焦点；支持中途反向，减少动态效果时立即切换。使用收起按钮或 Esc 返回原页面；队列打开时 Esc 优先收起队列，Ctrl+F 在收起完成后聚焦搜索。
- 歌词使用现有行级时间戳，不伪造逐字高亮；自然切句使用独立的逐行阻尼弹簧、错峰跟随及文字缩放，seek / 大幅跳转直接定位。支持翻译、点击定位、上下方向键定位、手动浏览与“返回当前歌词”。可切换简洁歌词模式；减少动态效果时不运行弹簧动画。
- 背景先对 48×48 封面做一次预模糊，再绘制到最大 480×320 的单个 Canvas；最高 24 FPS、不随高 DPI 增加像素预算，并缓存最近 6 张纹理。背景仅在播放且页面可见时流动，暂停、过渡期间或减少动态效果时保持静止；无封面、图片失败或跨域读取受限时使用默认渐变。
- 播放页过渡仅使用位移与透明度，不同时缩放/变形圆角；歌词与背景初始化延至展开完成，过渡期间暂停显示时钟和内部动画。行级显示时钟使用 20 Hz，歌词弹簧仍按屏幕帧率更新；布局几何在数据、窗口尺寸及字体变化时缓存，屏幕外歌词保留滚动空间但跳过内容绘制与模糊。
- 播放页独立实现，不包含 AMLL 运行时或其源码。
- 直接运行原生二进制时仍需安装 GStreamer 解码插件，尤其是 `gstreamer1.0-libav`。缺少插件可能出现 `No suitable plugins found` 并触发失败跳歌；AppImage 应自带所需插件。

## 检查与构建

```sh
cargo test -p music-core
cargo check -p netease-cloud-music-desktop
npm run build
npm test
npx playwright install chromium
npm run test:e2e
npm run tauri build -- --bundles deb
# 包含 WebKitGTK 与 GStreamer 的便携包
npm run tauri build -- --bundles appimage
```

核心集成测试生成 WAV，通过真实 GStreamer 管线和同步 `fakesink` 验证播放、自动切歌、定位与持久化，不会输出声音。浏览器测试使用隔离的 IPC 测试数据，验证界面交互；它不代替原生桌面和真实网络验收。

### 原生验证

```sh
# 实际 WebView、MPRIS、单实例、30 次销毁/重建
npm run build
cargo build -p netease-cloud-music-desktop --features native-smoke,custom-protocol
dbus-run-session -- python3 scripts/native-smoke.py
# 可选的两小时后台循环与进程组 PSS/CPU 采样
NCM_NATIVE_SOAK_SECONDS=7200 dbus-run-session -- python3 scripts/native-smoke.py
# 实际发行程序的页面、网络数据、二维码与播放交互
cargo install tauri-driver --locked
dbus-run-session -- python3 scripts/native-webdriver.py
# 可选：真实网络播放与缓存重放；CI 使用 NCM_TEST_OFFLINE=1
NCM_TEST_NETWORK=1 NCM_TEST_CACHE=1 dbus-run-session -- python3 scripts/native-webdriver.py
```

脚本自动创建隔离数据与 Xvfb 显示。`NCM_TEST_OUTPUT` 指定报告目录，`NCM_TEST_BINARY` 指定 WebDriver 待测二进制。
原生生命周期测试的 `native-smoke` feature 仅用于验证；发行构建不包含此功能。
真实 KDE、账户与安装升级步骤见 [MANUAL-ACCEPTANCE.md](MANUAL-ACCEPTANCE.md)。

## 数据与迁移

- 新配置：`$XDG_CONFIG_HOME/netease-cloud-music-desktop/`（默认 `~/.config/`）。
- 新会话：`$XDG_DATA_HOME/netease-cloud-music-desktop/`（默认 `~/.local/share/`）。
- 复用缓存：`$XDG_CACHE_HOME/netease-cloud-music-gtk4/`。
- 首次启动导入旧 `playlist.json`、`cookies.json` 和已安装 GSettings schema 的设置；旧文件保留。
- 使用 `NCM_DATA_DIR=/path/to/isolated-data` 可完全隔离新旧数据，适用于测试。
- 窗口关闭后保留播放进程，通过托盘、MPRIS Raise 或再次启动恢复窗口。
- 使用托盘“退出”或 Ctrl+Q 完整退出。

## 工程结构

```text
crates/music-core/  API、会话、播放服务、队列、歌词、持久化
src-tauri/         IPC、窗口生命周期、Linux 托盘/MPRIS、Windows 原生托盘
web/               页面与组件、展示状态、主题
tests/web/         浏览器交互回归
scripts/           构建依赖与检查入口
```

验证结果和未完成事项见 [VALIDATION.md](VALIDATION.md)，逐项功能对照见 [FEATURE-MATRIX.md](FEATURE-MATRIX.md)。

## 代码来源与许可

项目以 GPL-3.0-or-later 发布，详见 [COPYING](COPYING)。播放核心、队列及部分桌面集成源自 [gmg137/netease-cloud-music-gtk](https://github.com/gmg137/netease-cloud-music-gtk)，保留原作者与贡献者的版权声明；API 使用原作者的独立 Rust API 库。本仓库以当前 Tauri 版本建立独立的初始提交，不携带旧 GTK 项目的 Git 历史，这不改变复用代码的来源和许可证。

第三方组件与设计参考见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)。
