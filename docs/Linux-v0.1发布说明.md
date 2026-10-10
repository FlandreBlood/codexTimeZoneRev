# Linux v0.1 发布与使用

本 fork 的 Linux 版本独立从 v0.1 开始，基于上游提交 `2acc9cdc7883b63890488bc01d149304858f8da3`（v0.2.1）。Flutter 应用版本为 0.1.0+1，Rust 原生库为 0.1.0；版本区别不代表回退上游功能。

## 下载和启动

在 [v0.1 Release](https://github.com/FlandreBlood/codexTimeZoneRev/releases/tag/v0.1) 下载 .deb 和 SHA256SUMS，执行 `sha256sum --ignore-missing -c SHA256SUMS`，然后 `sudo apt install ./CodexTimeZone-Linux-amd64-v0.1.deb`。在应用菜单搜索“Codex 时区启动器 (Linux)”或运行 `codex-timezone`；关闭后用同一入口重新打开。不自动启动、不修改原 Codex 的入口。卸载用 `sudo apt remove codex-timezone`，用户设置保留。

另提供 tar.gz：校验、解压后执行包内 `bash run.sh`。包可移动到其他目录，运行不依赖构建机器的绝对路径。关闭后再次运行同一脚本；不需要重新编译。源码目录的现有启动方式仍是 `bash scripts/run-linux.sh`。

保存工作并完全退出已有 Codex，再从启动器“保存并启动”。沿用已安装客户端的原入口环境、代理、参数、工作目录及用户数据；不新建独立登录 profile。只给新客户端设置 TZ，不修改系统时区、不关闭已有客户端。Linux 不提供皮肤或提权修复。

## 发布包与兼容范围

发布附件为主安装包 `CodexTimeZone-Linux-amd64-v0.1.deb`、可选 `CodexTimeZone-Linux-x86_64-v0.1.tar.gz`、`SHA256SUMS` 和 `validation.json`。安装包把程序放在 `/opt/codex-timezone`，提供 `/usr/bin/codex-timezone`、应用菜单和图标；没有安装/卸载维护脚本，也不操作原客户端。包仅包含程序、Flutter 资源、原生库、所需构建运行库、启动脚本、使用说明和第三方许可/来源信息；不含本机设置、登录凭据、聊天数据、日志或完整 conda 环境。

打包器通过 ldd 收集构建工具环境依赖，复制库并把 ELF RPATH 改为相对路径，生成文件摘要与依赖清单。GTK 输入法模块和查询工具随包提供，启动时在启动器缓存目录生成当前路径的模块索引；默认使用系统字体配置，避免动态加载原 conda 路径。原客户端启动前清除该私有模块索引和缓存标记，再应用原桌面入口设置。系统 glibc、图形驱动、字体配置和系统桌面服务仍由目标机器提供。当前发布包要求 glibc ≥ 2.39。已在本设备 Ubuntu/KDE x86_64 验证，未验证其他发行版；Release 附件 validation.json 记录实际 glibc 要求及检测结果。

本机安装的 `/usr/lib/chatgpt/ChatGPT` 按 XDG 顺序读取原 chatgpt.desktop；其他安装路径使用经验证的桌面 ELF，其原入口复用尚未覆盖。公网信息面板对 SOCKS 代理的限制与客户端继承原代理是两条不同路径，详见 [Linux 运行说明](Linux运行说明.md)。

## 构建和验证

先运行 `bash scripts/flutter-linux.sh test`、`bash scripts/flutter-linux.sh build` 和 `python scripts/test-native-linux.py`。准备 Python、patchelf 和 dpkg-deb 后运行 `python scripts/package-linux-release.py --version v0.1`；本机 rootless 工具位于 environment，均忽略提交。

运行 `python scripts/test-linux-release.py` 校验两种格式；在正常桌面会话加入 `--gui`，只创建并关闭临时测试启动器。2026-10-09 发布验收：Rust 18 项通过，Flutter analyze 无问题、53 项通过/1 项跳过，原生 ABI 通过；deb 的 0.1.0/amd64 元数据、root 所有权、无维护脚本检查通过；两种包在含空格临时路径解压后的文件摘要、依赖解析和实际图形进程映射均通过，没有加载原构建目录运行库。未执行 sudo 系统安装或其他设备测试，不把解压运行等同于所有设备的 apt 安装验收。

验收区分单元测试/静态分析、隔离原生 ABI、本机图形启动、用户反馈与其他设备兼容性。登录复用成功来自用户现场反馈，详见 [登录修复记录](Linux登录状态修复.md)；发布时另检查解压目录的运行库与图形进程加载，不读取账户数据。

原 Windows/macOS 工作流保留源码，但在本 Linux fork 跳过，避免发布 v0.1 时附加未验收的其他平台安装包。GitHub main 与 v0.1 标签绑定同一发布提交，上传后核对附件名称、大小和服务端摘要。
