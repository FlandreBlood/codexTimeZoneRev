# Linux 本机适配

本地分支 `codex/linux-support`，基于上游 `2acc9cdc7883b63890488bc01d149304858f8da3`。目标是本设备 Ubuntu/KDE x86_64。皮肤功能按用户要求省略；Linux 不执行提权修复或外部皮肤脚本，不开放客户端调试端口。

## 运行和构建

在仓库目录中运行：

```bash
bash scripts/flutter-linux.sh test
bash scripts/flutter-linux.sh build
python scripts/test-native-linux.py
bash scripts/run-linux.sh
```

在桌面终端启动，沿用当前 DISPLAY / WAYLAND_DISPLAY / XDG_RUNTIME_DIR / XAUTHORITY。不要把图形会话鉴权文件内容复制到文档或版本控制。`preview` 为演示；`run` 为开发运行；`run-linux.sh` 使用已构建的 release。

本机缺少系统开发包且不能免密 sudo，因此使用隔离的 `environment/linux-tools` conda 环境（conda-forge：cmake、ninja、pkg-config、clangxx_linux-64、gtk3、glib、zlib、expat）。Flutter 3.47.5 官方压缩包的 SHA256 已对照官方发布元数据验证：`2132e990f236f8d22e7c6314b29a191a95b10d7cbcfec9b4e2e303d996652cbb`。SDK、构建目录、依赖及产物不提交。

也可自行安装官方 Linux 构建工具链；脚本优先使用项目内工具，找不到则使用 PATH。当前构建脚本针对 x86_64。conda 编译器使用平台前缀名称，脚本通过 CMake 显式指定其路径和链接时的依赖查找目录；本机构建同时写入项目工具环境的运行库查找路径，使应用菜单入口可直接启动。CMake 安装位置固定在仓库内的 bundle，重试不写入 `/usr/local`。该产物依赖本机环境，归档不视为任意 Linux 设备的通用安装包。

本 fork 的 v0.1 Release 另用 `scripts/package-linux-release.py` 收集运行库并修改相对 RPATH，生成 .deb 主安装包、可选 tar.gz 和摘要，不再依赖原构建目录。GTK 模块缓存仅用于启动器，不传给原客户端；发行包要求和实测边界见 [Linux v0.1 发布说明](Linux-v0.1发布说明.md)，原源码目录启动方式不变。

## 客户端与配置

- 自动发现本机 `/usr/bin/chatgpt` 包装入口，并直接使用 `/usr/lib/chatgpt/ChatGPT` 桌面 ELF；其他已列出的 `/opt` 安装位置也会检查。
- 手动选择仅接受有 `resources/app.asar` 的 ChatGPT/Codex 桌面 ELF，不接受 CLI、任意脚本、setuid/setgid 或其他用户可写的目标/目录。该检查不是发行者签名认证；应从可信安装来源选择客户端。
- 设置在 `$XDG_CONFIG_HOME/com.fei-away.codextimezone/settings.json`，未设置 XDG_CONFIG_HOME 时使用 `~/.config/com.fei-away.codextimezone/`。目录需为当前用户私有 700，文件为私有普通文件；拒绝符号链接和硬链接。
- 仅新进程设置 TZ；不改变系统时区。检测到现有客户端时停止，不关闭或重启已有进程。真实客户端时间显示须在保存工作、完全退出后用新启动实例确认。
- 启动去掉 `ELECTRON_RUN_AS_NODE`、`LD_LIBRARY_PATH`、`LD_PRELOAD`，避免启动器 conda/动态加载环境污染本机客户端；保留其他用户环境及网络代理。
- 本机客户端优先沿用实际生效的用户级 `chatgpt.desktop`，其次系统入口，传递其中的代理、输入法、客户端参数和工作目录；不支持或不安全的入口停止启动。检查结果只输出环境名称和一致性，不输出完整环境值。
- 启动时补齐缺失的桌面会话标识与 D-Bus 变量，使 Electron 沿用正常桌面入口的凭据存储。只接受当前 UID、相同图形/D-Bus 会话中的已知桌面进程，按白名单补齐；不覆盖显式变量，不复制 HOME、CODEX_HOME、XDG_CONFIG_HOME 或凭据变量。缺少可靠桌面身份、KDE 版本或 D-Bus 时拒绝启动。
- 沿用原 Codex 的登录、聊天、项目和设置：不自行添加 `--user-data-dir` / `--profile` / `--password-store` 参数，原入口已有显式设置保持；不迁移、复制、删除登录数据。登录问题的证据及剩余现场验收见 [登录状态修复记录](Linux登录状态修复.md)。
- 创建快捷方式写入 `$XDG_DATA_HOME/applications/`（默认 `~/.local/share/applications/`），可在应用菜单搜索。已存在时拒绝覆盖。
- 启动器公网信息面板默认不自动查询；手动刷新时逐块限制 1 MiB、超时 9 秒，拒绝 HTTPS 降级。该面板仅支持无认证 HTTP 代理，SOCKS、认证代理、KDE PAC 报错而不静默直连。客户端启动另行沿用原桌面入口代理，包括本机已有的 SOCKS5 配置；两条网络路径不同。

## 安全和验证

上游源码审计见 [安全审计与运行边界](安全审计与运行边界.md)。2026-10-08 本机验证结果：

| 检查 | 结果与边界 |
| --- | --- |
| Rust 单元测试 | v0.1 发布检查 18 项通过，覆盖时区偏移、目录/链接拒绝、客户端路径检查、现有进程保护、原入口环境/参数/目录、桌面会话补齐和发布包输入法缓存隔离 |
| Flutter 静态分析与测试 | analyze 无问题；53 项通过，1 项已有平台相关外观测试跳过；保留原 pubspec.lock 版本 |
| Release 构建 | 成功，`flutter_app/build/linux/x64/release/bundle/codex_timezone`；归档约 11 MiB，位于 `environment/artifacts/` |
| 运行库 | 不额外设置 LD_LIBRARY_PATH 时，ldd 无 not found；运行仍依赖此仓库内的 conda 工具环境 |
| 原生 ABI 验收 | 临时目录与隔离 ELF 测试进程通过 bootstrap、validate、save、launch、check_launch；子进程收到 America/New_York，父进程 TZ 不变，无额外客户端参数，ELECTRON_RUN_AS_NODE 被移除；第二次启动拒绝，测试只清理自身 fixture PID |
| 本机客户端保护 | 自动识别 `/usr/lib/chatgpt/ChatGPT`；check_launch 确认正在运行，launch 返回现有客户端拒绝提示；没有终止或重启它 |
| 实际图形窗口 | Ubuntu/KDE 的 XWayland 会话打开 1280×720 可见窗口；窗口截图确认设置已读取、客户端已自动找到、时间预览正常且无皮肤入口；使用真实 NativeBackend，不是 preview |

日志和仅含启动器自身窗口的截图保存在忽略提交的 `environment/linux-*.log` 与 `environment/linux-window.png`。启动日志有一条 ATK accessibility bridge warning，窗口渲染正常；辅助功能交互尚未验收。公网查询、真实文件选择器、实际应用菜单启动和其他 Linux 发行版尚未做现场验证。

真实 Codex 窗口的最终时间显示尚未确认，不能把 fixture/进程检查等同于该项验收。保存工作并完全退出已有客户端后，在仍打开的启动器内选择所需时区并点击“保存并启动”，检查新客户端中的时间显示。地区模式遵循夏令时；需要恒定偏移时改用 UTC 偏移模式。启动失败时保留原入口作为回退。

回退：关闭本启动器，使用原有 ChatGPT/Codex 入口启动客户端即可；系统时区和安装文件不被修改。删除应用菜单入口前先核对其 Exec 指向本启动器；配置和 environment 均为本机独立文件。

## 旧版与快捷方式清理

2026-10-08 按用户要求删除旧版。旧的运行文件已由修复版替换；剩余旧版备份 `environment/backups/20261008T080410Z-login-context/` 已删除。删除前确认当前运行程序与最新 bundle/归档中的原生库一致。

已删除 `~/.local/share/applications/com.fei-away.codextimezone.desktop` 和 `~/Desktop/com.fei-away.codextimezone.desktop`，并成功刷新 KDE 应用菜单缓存。这两个入口此前都指向同一个已更新的程序，不是额外安装的旧版。

当前修复版、最新归档、工具设置、原 Codex 安装/登录/聊天/项目数据及 `/usr/share/applications/chatgpt.desktop` 均保留。新版仍可通过 `bash scripts/run-linux.sh` 启动；不会自动重建快捷方式。
