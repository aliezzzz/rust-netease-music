# 0.1.0 Linux 候选包（历史构建记录）

构建日期：2026-10-22。平台：Ubuntu 26.04.1 / x86_64。以下校验值与测试结果仅对应当时构建，不代表包含后续播放页、性能与 Windows 适配改动；发行当前版本必须重新构建与验收。目录整理后路径相对于仓库根目录，现有本地产物不纳入 Git。功能状态见 [FEATURE-MATRIX.md](FEATURE-MATRIX.md)。

## 产物

相对于仓库根目录：

| 格式 | 路径 | 大小 |
| --- | --- | --- |
| Debian | `target/release/bundle/deb/netease-cloud-music-desktop_0.1.0_amd64.deb` | 8,525,440 字节 / 8.13 MiB |
| AppImage | `target/release/bundle/appimage/netease-cloud-music-desktop_0.1.0_amd64.AppImage` | 165,374,456 字节 / 157.71 MiB |

SHA-256：

```text
6a25bbccda73ea6dfba8ab51a2a4597ee24087d03c9c5f9979c1f659e756f202  deb/netease-cloud-music-desktop_0.1.0_amd64.deb
91f74974269eb8617c01080ac416fbc20fdfcb3e89ffa7122ec4f06ba5c22de3  appimage/netease-cloud-music-desktop_0.1.0_amd64.AppImage
```

## 直接试用

从仓库根目录运行：

```sh
NCM_DATA_DIR="$HOME/.local/share/ncm-desktop-acceptance" \
./target/release/bundle/appimage/netease-cloud-music-desktop_0.1.0_amd64.AppImage
```

若没有可用 FUSE，在命令前添加 `APPIMAGE_EXTRACT_AND_RUN=1`。AppImage 包含 WebKitGTK、GStreamer 和 libav，无需本次开发用的 `/tmp/opencode/` SDK 或插件路径。正常试听不要设置 `NCM_AUDIO_SINK=fakesink`。

Debian 安装：

```sh
sudo apt install ./target/release/bundle/deb/netease-cloud-music-desktop_0.1.0_amd64.deb
NCM_DATA_DIR="$HOME/.local/share/ncm-desktop-acceptance" netease-cloud-music-desktop
```

当前包在构建机器上通过验证；跨发行版兼容仍待验收。直接二进制引用 `GLIBC_2.39`，AppImage 内其他运行库可能有更高版本要求，不能作为旧发行版通用包。CI 使用 Ubuntu 24.04 从源码构建，尚未在远端执行。

## 候选包验证

- 格式检查、TypeScript/Vite 构建、全 features Clippy 通过。
- Rust 模块测试 12 项、真实 GStreamer 集成测试 1 项、前端同步测试 2 项、Playwright IPC fixture 测试 4 项通过。
- Xvfb 原生测试验证无窗口自动切歌、MPRIS、单实例及 30 次窗口重建；恢复设置页、队列面板和播放器状态。
- 从最终 `.deb` 解包的程序通过真实 WebKit 页面、网络播放、搜索封面、二维码、MPRIS seek、托盘及退出验证。
- 最终 AppImage 清空临时库/插件路径，以 extract-and-run 通过同一测试。
- 两种包都验证了下载后停止提交缓存，并在 API 代理不可达时重放缓存和定位至曲尾。测试网络歌曲 `2652820720` 缓存为 3,397,048 字节。
- HTTP 回归逐字节验证完整缓存，拒绝短试听和未完成下载；慢 HTTP 源停止回归通过。
- `.desktop` 校验、Debian 依赖求解模拟通过。实际系统安装与升级待用户。

较早构建已通过两小时无 WebView 后台长测（7200 秒、1197 次本地 WAV 循环、fakesink）；该结果不替代最终候选包的长期实际网络播放验收。完整范围和性能数据见 [VALIDATION.md](VALIDATION.md)。KDE / Wayland、扬声器、账户业务按 [MANUAL-ACCEPTANCE.md](MANUAL-ACCEPTANCE.md) 执行。

## 本轮关键修复

- 修复 GStreamer 快速切歌时的释放竞态。
- 修复下载完成通知早于文件缓冲关闭导致的缓存截断。
- 保证随机队列显示顺序与实际播放顺序一致，并限制连续推荐队列累积。
- 保存关闭窗口前的页面状态，丢弃过期页面/账户请求结果。
- 补齐搜索封面和收藏状态，保护设置草稿不被音量事件覆盖。
- 补充新旧 GStreamer Play 运行库包名的 Debian 依赖兼容。
