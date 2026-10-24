# KDE 与账户验收

自动化记录见 [VALIDATION.md](VALIDATION.md)。本清单用于用户已选择稍后执行的真实桌面验收。

## 启动

以下命令均从仓库根目录运行。已有候选包对应 [RELEASE.md](RELEASE.md) 中的历史构建，不包含之后的改动；验收当前代码应先安装依赖，再执行 `npm ci` 和 `npm run tauri build -- --bundles deb`。

```sh
# 安装会由 apt 检查并补齐运行依赖
sudo apt install ./target/release/bundle/deb/netease-cloud-music-desktop_0.1.0_amd64.deb
# 首轮测试使用独立数据，退出旧/新播放器后执行
NCM_DATA_DIR="$HOME/.local/share/ncm-desktop-acceptance" netease-cloud-music-desktop
```

本次无 sudo 构建的二进制也可直接试运行：

```sh
LD_LIBRARY_PATH=/tmp/opencode/ncm-sysroot/usr/lib/x86_64-linux-gnu \
NCM_DATA_DIR="$HOME/.local/share/ncm-desktop-acceptance" \
./target/release/netease-cloud-music-desktop
```

不设置 `NCM_AUDIO_SINK`，以便使用实际音频输出。临时 SDK 路径只适用于本次开发机器。

### 无需安装的 AppImage

```sh
NCM_DATA_DIR="$HOME/.local/share/ncm-desktop-acceptance" \
./target/release/bundle/appimage/netease-cloud-music-desktop_0.1.0_amd64.AppImage
```

若系统没有可用的 FUSE，可在同一命令前增加 `APPIMAGE_EXTRACT_AND_RUN=1`。AppImage 已包含 WebKitGTK 和 GStreamer 插件；本机验证不代表所有发行版兼容，当前产物在 Ubuntu 26.04 构建。

## 播放与桌面

- [ ] 搜索并播放至少两首实际可用歌曲，确认有声音；暂停、上一首、下一首、进度和音量正确。
- [ ] 歌曲快结束时关闭窗口，确认继续播放并自动切下一首。
- [ ] KDE 媒体控制 / 键盘媒体键可暂停、继续、切歌、seek；曲名、封面、时长和进度正确。
- [ ] 点击托盘打开，页面、队列、进度和歌词恢复；再次启动不会出现第二个播放器。
- [ ] 托盘退出 / Ctrl+Q 后进程和声音均停止；重新启动恢复队列并保持停止状态。
- [ ] Wayland、100% / 150% / 200% 缩放下文字、封面和控件正常；小窗口无横向溢出。
- [ ] 浅色、深色和系统主题切换正常；音频输出设备切换后仍可播放。

## 沉浸式播放页（本轮原生待验收）

本轮已通过前端构建和隔离 IPC 浏览器测试，未替代实际 WebKitGTK / 音频验收。

- [ ] 从底部封面 / 歌词按钮展开，侧栏与底栏不可误操作；关闭后恢复原路由、滚动和键盘焦点。
- [ ] 真实封面加载后背景流动；无封面、图片失败仍有默认背景。暂停、隐藏窗口及减少动态效果时停止动态效果。
- [ ] 真实 LRC 与翻译跟随正确；快速连续切歌不会出现旧歌词；seek、暂停 / 恢复不会发生时间漂移。
- [ ] 手动浏览歌词不会被强行拉回，点击歌词与“返回当前歌词”可恢复跟随；超长歌词、中英文混排无截断。
- [ ] 播放页中的喜欢、音质、音量、随机 / 循环与底栏一致；队列浮层可切歌；Esc 优先关队列再关页面，Ctrl+F 回到搜索。
- [ ] 最小窗口、高 DPI、系统减少动画设置正常；连续开关播放页 30 次，关闭后没有新增动画循环或持续内存增长。

## 账户与资料库

- [ ] 扫码登录（含过期刷新、手机确认），重启后保持会话；验证码登录单独验证。
- [ ] 每日推荐、推荐歌单、喜欢、我的歌单、收藏专辑、云盘及订阅电台与账户一致。
- [ ] 喜欢 / 取消喜欢歌曲，收藏 / 取消收藏歌单和专辑，刷新后状态一致。
- [ ] 私人 FM 连续多批推荐、下一首、不喜欢；关闭窗口后仍连续推荐。
- [ ] 心动模式可开启和关闭，推荐耗尽后继续；与普通随机 / 循环切换正常。
- [ ] 退出登录后不再显示之前的喜欢状态；重启不会重新导入旧登录。
- [ ] 标准和账户可用的高音质播放正常；不可用音源有明确反馈并能跳过。

## 兼容与性能

- [ ] 备份旧配置后，不设置 `NCM_DATA_DIR` 启动，核对旧队列、设置与登录导入。
- [ ] 缓存复用、清理与歌词导出工作正常；下载完成的歌曲在停止/切歌/退出时提交完整缓存，短试听不保存为完整版；用户数据不会因升级丢失。
- [ ] 在同一音源和桌面上比较旧 GTK、YesPlayMusic、新版前台、隐藏和销毁窗口后的进程组 PSS/CPU。
- [ ] 实际音源连续播放两小时，期间切歌、锁屏、网络恢复及重开窗口正常。

记录发行版、Plasma 版本、X11/Wayland、缩放、声音后端和失败步骤。不要在问题报告中粘贴 cookies 或验证码。
