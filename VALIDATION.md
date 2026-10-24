# 验证记录

下文主体记录 2026-10-22 的 Linux 迁移候选包，不代表后续改动已重新打包。2026-10-24 已完成沉浸式播放页与 Windows 适配的本机回归，详见 [WINDOWS.md](WINDOWS.md) 的验证边界；目录整理后所有脚本入口相对于仓库根目录。

旧 GTK 应用源码、Meson 与旧发行工作流已移出当前项目。保留以下历史验证证据，以及必要的兼容性与来源说明。

## 2026-10-24 根目录整理回归

- 原 `desktop/` 应用提升到仓库根目录，Linux / Windows CI 的工作目录、依赖缓存与产物路径同步更新。
- 根目录运行 `cargo test --workspace --locked`：12 项核心测试、1 项真实 GStreamer 集成测试、1 项运行库布局测试通过。
- `cargo fmt --all -- --check`、全 targets / features Clippy 通过。
- 前端 14 项单元测试、16 项浏览器回归、TypeScript 检查和 Vite 生产构建通过。
- 未在此次目录整理中重新生成 Linux / Windows 安装包；历史候选包仍不代表当前源码。

## 2026-10-22 已执行

- Vue TypeScript 检查与 Vite 生产构建通过。
- 前端状态同步测试 2 项通过（快照/订阅竞态、进度事件保留队列）。
- Playwright 浏览器交互测试 4 项通过：导航/播放器/歌词/队列同步；一万首歌曲虚拟列表；搜索失败、键盘和窄窗口深色布局；收藏切换、设置草稿在音量更新时保持及注销后的 UI 状态（IPC fixture）。
- Rust 核心模块测试 12 项通过：队列模式、删除/追加、随机顺序显示、损坏顺序修复、旧队列迁移、并发原子写入、损坏状态文件保护、歌词时间与翻译、cookie 迁移/去重/作用域/空会话阻止重复导入、连续推荐批次替换和旧结果丢弃，以及真实 HTTP 缓存完整性/试听拒绝/慢源停止。
- 真实 GStreamer 集成测试通过：无窗口播放、自动下一首、暂停、seek、音量、退出与恢复。音频输出为测试用 fakesink。
- 快速选曲与管线销毁竞态曾触发 GLib / GStreamer 严重警告；调整释放顺序后，`G_DEBUG=fatal-criticals` 下连续 30 次回归通过。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过。
- Tauri 原生编译、Release 编译及 Debian 打包通过；包名采用合法 ASCII 名，中文桌面名称保留，`desktop-file-validate` 通过。
- Xvfb 原生测试通过无窗口自动下一首、MPRIS Play/Pause/Raise、第二实例唤起及退出、30 次真实 Vue DOM / IPC 销毁重建。最新回归同时断言每次恢复设置页面和展开的队列面板，`G_DEBUG=fatal-criticals` 下通过。
- 真实网易云发现、新专辑、排行榜、歌曲/歌手/专辑/歌单/歌词搜索通过；歌曲搜索封面从 `picId` 补齐，探针得到 6/6 封面地址。歌单/专辑/歌手详情分别返回 97/1/50 首，歌词返回 35 行。
- 真实网络音源 `2652820720` 经 GStreamer 解码并播放超过 2 秒，检测时长 278962 ms，写入 3393732 字节缓存。只验证了部分播放与缓存提交，没有据此声称完整歌曲收听完成。
- 从最新 Debian 包解出的程序经 WebKitWebDriver 验证发现、搜索及封面加载、二维码、队列选曲、暂停、MPRIS SetPosition、主题保存与正常退出。包内程序还通过真实网络歌曲播放。
- Debian 包约 8.13 MiB；`apt-get -s install` 依赖求解成功。未实际安装至系统或执行升级。
- StatusNotifier 测试宿主通过真实 D-Bus 协议验证注册、菜单播放/暂停、Activate 与退出；尚非 KDE 面板实测。
- 最终包含 libav 的 AppImage 打包成功（157.71 MiB），清空临时 SDK `LD_LIBRARY_PATH` 与外部 `GST_PLUGIN_PATH` 后，以 extract-and-run 启动并通过真实 UI / WAV / 网络播放 / 封面 / 托盘测试。
- 缓存逐字节测试发现“下载完成”通知早于 GStreamer 写缓冲关闭：64044 字节 WAV 曾仅保存 61354 字节。现保留源文件句柄，在管线关闭后提交缓存，逐字节对比、短试听拒绝和未完成下载清理均通过。早期网络缓存大小记录不能证明当时缓存完整。
- 最终 `.deb` 和 AppImage 均验证真实音源缓存重放及曲尾定位，重放时 API 代理设为不可达。歌曲 `2652820720` 的最终缓存为 3397048 字节。
- 交付路径、大小及 SHA-256 记录于 [RELEASE.md](RELEASE.md)。

## 阶段性性能与长时间测试

- 最终候选代码的窗口恢复回归中，第 5 次关闭后的主进程 PSS 为 115160 kB，第 30 次为 120335 kB；末尾后台进程组共 2 个进程，PSS 139819 kB。这些是瞬时值，不是稳态无泄漏结论。
- 最终回归真实 Vue DOM / IPC / 页面恢复耗时最小 1056、中位 1058、最大 1209 ms；测试每 150 ms 采样。早前与 Release 编译同时进行的回归为 1.06–2.42 秒，不能当作同负载性能基准。
- 两小时无 WebView 后台循环测试已通过：`SOAK_OK: 7200 seconds, 1197 loops without WebViews`、`NATIVE_SMOKE_OK`，退出码 0，`result.txt` 为 `PASS`。启用 `G_DEBUG=fatal-criticals`，使用本地 WAV / fakesink。测试二进制早于本轮缓存、封面与页面恢复修复，不能据此声称最终包两小时真实网络或扬声器验证通过。
- 长测后台统计排除前 120 秒预热及最后 10 秒，共 1415 个采样，应用及后代进程数始终为 2。总 PSS 首个/最后采样为 147910/149602 KiB（约 144.44/146.10 MiB），增量 1692 KiB（约 1.65 MiB）；最小/中位/最大为 136925/148774/149602 KiB。平均 CPU 约占一个逻辑核心的 0.948%。PSS 会受共享页分摊影响，这些数据不构成零泄漏或优于旧版的结论。
- `scripts/native-smoke.py` 采集进程组 PSS 与 CPU 时间；旧 GTK / YesPlayMusic / KDE 同条件比较仍待执行。

## 可复现入口与证据

| 验证 | 入口 / 本机产物 |
| --- | --- |
| 格式、核心、前端、浏览器 | `bash scripts/check.sh` |
| 原生生命周期 | `scripts/native-smoke.py`；`/tmp/opencode/ncm-native-soak/` |
| 两小时长测汇总 | [validation/native-soak-2026-10-22.json](validation/native-soak-2026-10-22.json)；`python3 scripts/summarize-native.py /tmp/opencode/ncm-native-soak` |
| 页面与队列恢复 | `/tmp/opencode/ncm-native-restoration/` |
| 最终候选生命周期 | `/tmp/opencode/ncm-native-final/`；`python3 scripts/summarize-native.py <证据目录>` |
| Debian 原生 UI 与托盘 | `scripts/native-webdriver.py`；`/tmp/opencode/ncm-native-tray/` |
| 最新 Debian 网络播放 / 封面 | `/tmp/opencode/ncm-release-latest/`、`/tmp/opencode/ncm-cover-check/` |
| 缓存修复后 Debian / 网络缓存重放 | `/tmp/opencode/ncm-deb-final-test/` |
| AppImage 原生 UI 与托盘 | 同一 WebDriver 脚本；`/tmp/opencode/ncm-appimage-test/` |
| 最终 AppImage / 网络缓存重放 | `/tmp/opencode/ncm-appimage-final-test/` |
| 真实业务 API | `cargo run -p music-core --example api_probe` |
| 真实网络播放 | `cargo run -p music-core --example network_playback`；`/tmp/opencode/ncm-network-v3/` |

上述临时目录为本次机器的原始验证证据，未纳入源码；两小时长测的统计摘要已保存到 `validation/`。CPU/PSS 统计范围为应用及其后代进程，不含 Xvfb 和会话总线启动的公共桌面服务。CI 配置位于 `.github/workflows/desktop.yml`，已建立但尚未在 GitHub 执行。
CI 所用 `NCM_TEST_OFFLINE=1` 原生 WebDriver 分支已在本机通过，证据目录 `/tmp/opencode/ncm-offline-ci/`；该分支不依赖发现/搜索等网络接口的断言。

## 待执行或尚未完成

- 账户功能真实验收：扫码确认、验证码、会话续期、收藏写入、云盘、订阅电台、私人 FM 与心动模式。
- KDE Plasma / Wayland 桌面与缩放实测。
- 最终候选包的长期实际网络播放及同条件内存/CPU 基线比较。
- 当前代码的 Linux 发行包重新构建、安装升级，以及 Windows MSVC 构建与真机验收。

用户选择 KDE / 账户部分稍后手动验收，步骤见 [MANUAL-ACCEPTANCE.md](MANUAL-ACCEPTANCE.md)。当前状态为可运行的 Tauri 客户端，尚非全平台最终验收完成。

## 环境

Ubuntu 26.04.1，Rust 1.98.1，Node 24.19。工具会话未连接 KDE 显示服务器。
临时 SDK 解包到 `/tmp/opencode/ncm-sysroot`，用于无 sudo 权限时验证构建。
测试依赖、临时 SDK 和图形测试数据不属于交付内容。
