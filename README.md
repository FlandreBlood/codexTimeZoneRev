# Codex 时区启动器 · Linux 版 v0.1

这是 [iUshio/codexTimeZoneRev](https://github.com/iUshio/codexTimeZoneRev) 的 **Linux 适配 fork**，使用 Flutter、Forui 和 Rust。上游版本基线为 `v0.2.1`，本 fork 的 Linux 版本独立从 **v0.1** 开始。项目并非 OpenAI 官方应用，保留上游作者与 [LICENSE](LICENSE)。

本版已在 Ubuntu/KDE x86_64 实测运行，用户确认原 Codex 登录状态复用成功。它为新启动的客户端设置指定时区，不修改系统时区；已有客户端须完全退出后重新启动。Linux 不提供皮肤功能。

## 主要功能

- 选择 120 个常用 IANA 地区时区（含夏令时规则）或 UTC−12 至 UTC+14 的固定偏移，预览本地与目标时间。地区清单重点覆盖美国、亚洲和欧洲，可按城市名称或 IANA 标识搜索；美国包含主要时区及亚利桑那、阿拉斯加、阿留申群岛、夏威夷等特殊地区。
- 自动发现或手动选择客户端，保存配置、检查启动条件并创建快捷方式（Linux 为应用菜单入口）。
- 查询双线路公网信息、缓存结果、复制 IP，并将可识别的时区用于设置草稿。公网查询会连接第三方服务。
- Windows/macOS 支持 Dream Skin 兼容启动、打开管理器和“重新注入皮肤”；Linux 省略此功能。重新注入需要当前客户端通过兼容模式启动并开放本机调试端口；此操作不会重启客户端或改变其时区。
- 读取旧版 `settings.json`；用户配置保存在本机，发布包不包含本机设置或日志。

## 下载与运行

### Linux v0.1 下载

从本 fork 的 [v0.1 Release](https://github.com/FlandreBlood/codexTimeZoneRev/releases/tag/v0.1) 下载 **`CodexTimeZone-Linux-amd64-v0.1.deb`** 和 `SHA256SUMS`，放在同一目录：

```bash
sha256sum --ignore-missing -c SHA256SUMS
sudo apt install ./CodexTimeZone-Linux-amd64-v0.1.deb
codex-timezone
```

安装后在应用菜单搜索 **Codex 时区启动器 (Linux)**，或运行 `codex-timezone`；关闭后用同一入口重开。卸载使用 `sudo apt remove codex-timezone`，用户设置保留。Release 另外提供无需安装的 tar.gz 包。

发布包附带本次构建使用的运行库，不需要原机器的 conda 目录；要求 **Linux x86_64、glibc ≥ 2.39**、系统图形驱动和正常桌面/D-Bus 会话，不承诺所有发行版兼容。已安装的 Codex/ChatGPT 客户端需自行提供。详见 [v0.1 发布与使用说明](docs/Linux-v0.1发布说明.md)。

Linux 复用原 Codex 实际生效的桌面入口，沿用代理、参数、工作目录和用户数据设置，不创建独立 profile；缺失的桌面会话变量会补齐。用户已确认登录复用修复实测成功，详见 [登录状态修复记录](docs/Linux登录状态修复.md)。

本 fork 的 Release 发布 Linux 包。Windows/macOS 下载与说明请参考 [上游项目](https://github.com/iUshio/codexTimeZoneRev)。本机源码构建与诊断见 [Linux 运行说明](docs/Linux运行说明.md)，风险与边界见 [安全审计](docs/安全审计与运行边界.md)。

### Windows 启动排错

对于 Microsoft Store / MSIX 安装的客户端，每次启动都会重新识别当前注册版本；设置中保存的旧 Store 版本路径会跟随更新。普通手动安装路径保持原选择。新进程在执行前核对程序包与应用身份。

若检测到受支持的 `15700 / 15703` 身份缺失，点击“一键修复并重试”，再确认 Windows 管理员提示。启动器先备份权限，仅将当前 Codex 包的一条已知异常普通用户规则从读取和执行缩小为读取，保留包身份条件执行权限、所有者及其他规则。完成后重新检查身份，并按原时区和皮肤设置重试一次。取消授权、权限不符合条件、版本变化或复检失败都会停止；不会关闭现有客户端。正常启动不会请求管理员权限。

权限备份、修复结果和日志保存在 `data/repairs/repair-*/`。需要回退时，核对该次 `plan.json` 后在管理员 Windows PowerShell 中运行同目录的 `repair_package_acl.ps1 -Mode Rollback -PlanPath <plan.json绝对路径>`；脚本仅接受原版本和符合备份的权限状态。回退可能恢复原启动故障。请勿重置整个 WindowsApps 的权限。

此流程用于在更新后再次出现已知权限问题时恢复启动，不保证未来客户端版本或未知权限变化均可自动修复。修复脚本内置在 DLL 中，完整发布包不依赖本机 diagnostics 文件夹。

启动阶段、目标版本、所选时区和原生错误会写入启动器旁的 `data/launch.log`，不记录完整环境变量。普通启动提示仅确认进程已启动；客户端窗口就绪及最终显示的时区仍需实际确认。

## 从源码构建

需要 Flutter 3.47.5（Dart 3.13.4）、Rust stable。Windows 还需要 Visual Studio C++ 桌面工具链；macOS 需要完整 Xcode。将 Flutter 的 `bin` 与 Cargo 加入 `PATH`，或在 Windows 传入 `-FlutterSdk`。

```bash
# Linux：安装 GTK 3 开发库、clang、CMake、Ninja、pkg-config 后
bash scripts/flutter-linux.sh test
bash scripts/flutter-linux.sh build
python scripts/test-native-linux.py
bash scripts/run-linux.sh
# 可选：制作包含运行库的 Linux 发布包，需要 Python 和 patchelf
python scripts/package-linux-release.py --version v0.1
# 检查 deb/tar.gz 元数据、文件摘要、解压运行库及测试窗口
python scripts/test-linux-release.py --gui
```

```powershell
# Windows：在仓库根目录运行
.\scripts\flutter-windows.ps1 -Action test
.\scripts\test-native-windows.ps1
.\scripts\test-package-acl-repair.ps1
.\scripts\test-repair-entry.ps1
# 可选：以安装 Codex 的用户验证当前包身份、旧路径跟随更新和过期修复拒绝；不执行客户端代码。
.\scripts\test-native-windows.ps1 -InstalledPreflight
.\scripts\flutter-windows.ps1 -Action build
```

```bash
# macOS：在仓库根目录运行
bash scripts/flutter-macos.sh test
bash scripts/flutter-macos.sh build
```

构建产物位于 `environment/artifacts/`。本 fork 的 Linux Release 从本机已验证产物发布；原 Windows/macOS 云端发布工作流仅在上游仓库执行。

## 项目结构

| 目录 | 内容 |
| --- | --- |
| `flutter_app/` | Flutter + Forui 界面、平台工程与测试 |
| `native/launcher_core/` | Rust 原生后端、跨平台逻辑与测试 |
| `scripts/` | 本地测试、构建与打包脚本 |
| `.github/workflows/` | 发布时运行的双平台构建工作流 |

开发模式、配置迁移和平台限制见 [运行说明](flutter_app/README.md)。历史迁移过程见 [迁移记录](FLUTTER_MIGRATION_PLAN.md)。项目按 [LICENSE](LICENSE) 授权。
