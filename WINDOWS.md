# Windows 支持与构建（首版适配）

目标：Windows 10/11 x64，MSVC 工具链，WebView2。**代码与 CI 流程已添加；在 Windows CI / 真机跑通之前，不视为已经发布或通过 Windows 验收。**

## 当前范围

- Windows 使用 Tauri 原生托盘：左键/双击恢复主窗口，右键菜单提供打开、上一首、播放/暂停、下一首、退出。曲名与播放状态同步，关闭 WebView 后托盘仍存在。
- 关闭窗口保存 UI 后销毁 WebView，音频核心保持运行；再次启动通过单实例回调恢复窗口。托盘退出/Ctrl+Q 保存播放状态并退出进程。恢复请求在 UI 线程串行处理，退出后不再创建窗口。
- Linux 继续使用 `ksni` + MPRIS；其依赖不参与 Windows 编译。Windows 不调用 `gsettings`。
- Windows 系统媒体面板、锁屏媒体控制与媒体快捷键（SMTC）**尚未实现**。不要把托盘播放按钮误认为 SMTC 支持。
- 配置：`%APPDATA%\netease-cloud-music-desktop`；会话：`%LOCALAPPDATA%\netease-cloud-music-desktop`；缓存沿用 `netease-cloud-music-gtk4` 目录名。`NCM_DATA_DIR` 可设置隔离目录。

## 构建依赖

1. Windows 10/11 x64，Visual Studio 2022 Build Tools：Desktop development with C++、Windows SDK。
2. Rust stable MSVC (`x86_64-pc-windows-msvc`)、Node.js 24。
3. WebView2 Evergreen Runtime（系统没有时安装程序会联网下载）。
4. Strawberry Perl、NASM，供 Windows vendored OpenSSL 与原生依赖构建；CMake 可由 VS 安装。OpenSSL 静态构建仅用于 Windows，不改变 Linux 依赖。
5. GStreamer **MSVC x64 runtime + development** SDK。不要使用 MinGW 或 32 位包。

从 **x64 VS 2022 Developer PowerShell** 执行（安装 SDK 时需要管理员权限）：

```powershell
# 在克隆后的 rust-netease-music 仓库根目录执行
./scripts/install-gstreamer-windows.ps1
# 固定 GStreamer 1.26.11，下载后验证仓库内固定的 SHA-256。
# 新开终端后需要恢复构建环境：
$env:GSTREAMER_1_0_ROOT_MSVC_X86_64 = 'C:\gstreamer\1.0\msvc_x86_64'
$env:PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\bin;$env:PATH"
$env:PKG_CONFIG_PATH = "$env:GSTREAMER_1_0_ROOT_MSVC_X86_64\lib\pkgconfig"

npm ci
./scripts/stage-windows-runtime.ps1
npm run tauri dev
```

`stage-windows-runtime.ps1` 需要 VS 开发终端的 `VCToolsRedistDir`。它复制 GStreamer runtime、全部插件、scanner、许可证和 VC Runtime 到忽略提交的 `src-tauri/windows-runtime/`，不修改系统 PATH，也不提交 DLL。

## 生成安装包

```powershell
npm run build
cargo test -p music-core --locked
cargo test -p netease-cloud-music-desktop --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
npm run tauri build -- --target x86_64-pc-windows-msvc --bundles nsis,msi --ci
```

Tauri 自动合并 `src-tauri/tauri.windows.conf.json`。产物位于：

- `target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe`
- `target/x86_64-pc-windows-msvc/release/bundle/msi/*.msi`

NSIS 默认按当前用户安装。当前未配置代码签名，SmartScreen 可能提示未知发布者；正式发行应配置签名并审核第三方运行库许可证。WebView2 缺失时需要网络，当前不是完全离线安装包。

### 运行库布局不可改变

```text
安装目录/
  netease-cloud-music-desktop.exe
  gstreamer-1.0-0.dll、glib DLL、VC Runtime、其他依赖 DLL...
  runtime/gstreamer/
    lib/gstreamer-1.0/        解码器、音频输出、网络等插件
    libexec/gstreamer-1.0/   gst-plugin-scanner.exe
    lib/gio/modules/        HTTPS/TLS 模块
    etc/ssl/certs/           CA 证书
    share/                  许可证与必要资源
    runtime-manifest.json
```

**Windows 在 `main()` 前就要解析导入的 DLL。只在 main 中追加 PATH 不足以解决核心 DLL 缺失。** 因而核心 DLL 安装到 EXE 同级；应用入口只负责把 scanner、插件搜索路径和独立 registry 指向打包资源，并避免混用系统 GStreamer。

不能只复制 EXE 到别处运行。Release 检测到打包资源不完整会终止启动。开发和测试使用 SDK 时也必须确保插件齐全。

## 原生与干净路径验证

```powershell
npm run build
cargo build -p netease-cloud-music-desktop --features native-smoke,custom-protocol --locked
./scripts/windows-smoke.ps1
```

脚本将程序与 runtime 复制到临时目录，先检查关键解码器/音频输出/HTTPS 插件并进行 AAC 编解码，再仅保留 Windows 系统 PATH 启动应用，使用独立数据和 `fakesink`（不发声），验证真实 WebView2 IPC、无窗口自动切歌、单实例恢复、托盘存在及 30 次销毁/重建。日志路径会输出到终端；失败时保留日志。它**不证明托盘鼠标操作、扬声器输出、真实账户或安装器升级正常**。

GitHub Actions 的 `windows` job 执行上述流程并上传安装包与日志。需要提交/推送后由 CI 真正运行；仅存在 workflow 不代表已经通过。

## 真机发布验收

在没有 GStreamer SDK、Perl、NASM、Rust 的 Windows VM 上：

- [ ] 分别安装 NSIS / MSI，检查核心 DLL 同级、插件和许可完整；首次启动正常。
- [ ] MP3、AAC、FLAC、ALAC 实际音源播放、seek、缓存重放与切换输出设备正常。
- [ ] 中文/空格用户名与安装路径正常；二维码登录与 HTTPS 证书校验正常。
- [ ] 左键/双击托盘恢复，右键菜单各操作正确；没有重复图标。
- [ ] 关闭窗口后继续播放；再次启动只有一个进程/窗口；最小化后可恢复。
- [ ] 托盘退出和 Ctrl+Q 真正结束音频与进程，重新启动恢复状态。
- [ ] 托盘菜单状态与前端同步；退出中重复点击恢复不会创建残留窗口。
- [ ] 100%/150%/200% 缩放、歌词进出与减少动态效果正常。
- [ ] 覆盖安装/卸载后用户配置不意外丢失；确认 VC Runtime 与 WebView2 行为。

## 已完成的本机验证边界

本次开发环境是 Linux，已验证：

- Rust workspace 测试：12 项核心单元测试 + 1 项真实 GStreamer 集成测试 + 1 项运行库布局测试；格式检查与全 features Clippy 通过。
- Linux 原生 smoke：MPRIS、无窗口播放、单实例恢复、30 次 WebView 销毁/重建通过。
- 前端 14 项单元测试、16 项浏览器回归及生产构建通过。
- Windows 目标依赖图不包含 GTK/WebKitGTK、ksni、MPRIS 或 D-Bus；以临时类型检查工程检查 Windows 托盘模块的 Tauri API（不等于 Windows ABI 构建）。
- Windows 配置通过锁定版本的 Tauri 配置解析；三个 PowerShell 脚本通过 PowerShell 7 语法解析。
- 固定 GStreamer runtime MSI 已下载并核验 SHA-256，确认 scanner、核心 DLL、libav 与 TLS 文件名称。

**尚未在 Windows 执行安装器、完整 MSVC 构建或原生 smoke**；以 Windows CI/真机结果为准。
