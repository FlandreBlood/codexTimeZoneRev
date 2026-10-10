# Linux 登录状态与原入口复用

## 用户实测与修正后的证据

2026-10-08 用户实测：从时区启动器启动 Codex 会要求重新登录，原应用菜单入口仍保持登录；补齐 KDE 会话后再次测试仍失败。之前的 KDE 凭据后端判断不足以解释问题，不能作为已修复结论。用户要求沿用原 Codex 的全部内容，不单开独立 profile。

本机实际生效的是 `~/.local/share/applications/chatgpt.desktop`，覆盖系统的 `/usr/share/applications/chatgpt.desktop`。其 Exec 为原客户端配置 HTTP_PROXY、HTTPS_PROXY、ALL_PROXY、NO_PROXY、XMODIFIERS 和 GTK_IM_MODULE；前三项使用本机 SOCKS5 代理。工具执行环境没有这些代理变量，桌面会话补齐也不会恢复桌面入口中的独立配置。已确认这是两种启动方式的实际差异，但尚未证明它是登录失败的唯一原因。

原包装脚本仅 exec `/usr/lib/chatgpt/ChatGPT`。原运行客户端的文件描述符路径指向 `~/.config/Codex`，没有观察到另一个 ChatGPT/codex-desktop profile。安装包静态代码默认使用 Codex 用户数据目录，允许 CODEX_ELECTRON_USER_DATA_PATH 显式覆盖；当前工具和桌面会话没有该覆盖。只检查路径和源码，没有读取文件描述符对应内容、cookie、令牌、auth.json 或密钥库。客户端启动后的 `/proc` 环境/参数已不能可靠解析，不能据此声称测得其完整启动环境。

已参考 [上游 macOS 启动代码](https://github.com/iUshio/codexTimeZoneRev/blob/2acc9cdc7883b63890488bc01d149304858f8da3/native/launcher_core/src/platform/mac/mod.rs)、[Desktop Entry Exec 规范](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html) 和 [Electron safeStorage](https://www.electronjs.org/docs/latest/api/safe-storage)。上游直接为原客户端进程设置 TZ，没有建立独立账户目录。

## 当前启动行为

- 对本机安装路径，按 XDG 顺序读取实际 chatgpt.desktop，优先用户入口，再检查系统入口；无入口或不支持的入口明确报错。
- 沿用原入口的显式环境、参数和工作目录。空 Path 使用用户 HOME；没有 URI/文件输入时移除独立的 %U/%u/%F/%f 字段。不通过 shell 执行 Exec，仅接受已核对的客户端命令别名，实际运行经验证的桌面 ELF。
- 最后设置所选 TZ，清除 ELECTRON_RUN_AS_NODE、LD_LIBRARY_PATH、LD_PRELOAD；其他原入口设置保持。原入口的桌面变量优先于启动器继承环境。
- 从同 UID、同图形/D-Bus 会话的已知桌面进程按固定白名单补齐缺失会话变量；缺少可靠桌面身份、KDE 版本或 D-Bus 时停止。不会从桌面进程复制凭据、HOME、CODEX_HOME、XDG 配置路径或 PATH。
- 不自行添加 profile、user-data-dir 或 password-store 参数；原入口已有的显式设置保留。不迁移、复制、删除或重置原 Codex 登录/聊天/项目/设置，不关闭当前 Codex。
- check_launch 返回入口路径、环境变量名称及环境/参数/目录一致性布尔值，不输出代理值、完整 Exec 或登录内容。

## 验证结果与剩余验收

2026-10-08 修复验证：

| 检查 | 结果 |
| --- | --- |
| Rust | 17 项通过；覆盖原入口环境/参数/目录、引号与字面量、shell/未知命令拒绝，以及桌面白名单、显式变量优先与不完整上下文停止 |
| Release 构建 | 成功；bundle 和本机归档已更新 |
| 隔离 ELF 启动 | 原生 ABI 验收通过，TZ、桌面上下文传递，HOME、CODEX_HOME、XDG_CONFIG_HOME 保持继承；只清理测试进程 |
| 本机原入口计划 | 真实 ABI 选择用户级 chatgpt.desktop；六项环境变量名称符合原入口，环境/参数/工作目录一致性均为 true；原客户端仍运行 |
| 原代理路线 | 本机 SOCKS5 端口接受无认证握手；只证明本地代理可用，不等于 OpenAI 登录成功 |
| 当前工具与清理 | 新版启动器已运行，进程映射当前 bundle 原生库并保持原 HOME/KDE 6 上下文；归档内程序和原生库与 bundle 摘要一致；旧备份和两个时区快捷方式仍不存在 |

本次脱敏结果保存在 `environment/linux-original-entry-*.log`，不会提交运行日志。Dart/UI 本次未修改，沿用此前 53 项通过、1 项平台测试跳过的结果。测试和启动计划检查没有执行真实客户端登录，不能替代账户验收。

2026-10-08 用户反馈“已经成功”，确认本次原入口复用修复后的登录问题现场复测成功。该结果来自用户实测；原聊天/项目逐项检查及最终时间显示没有单独确认。

日常使用：在桌面终端执行 `bash /home/bbb/workspace/codes/codexTimeZoneRev/scripts/run-linux.sh`，关闭后再次执行同一命令即可，不依赖终端所在目录。要应用所选时区，先保存工作并完全退出已有 Codex，再从启动器“保存并启动”。若异常，使用原应用菜单入口恢复，不点击清除数据或注销；既有客户端无法热切换启动环境。

## 清理与回退

按用户要求，旧版备份 `environment/backups/20261008T080410Z-login-context/` 和桌面/应用菜单中的两个时区启动器快捷方式已删除。本次不重建这些入口，只更新当前 bundle 与唯一最新归档。原 Codex 安装及原桌面入口保留；出现异常时关闭启动器，通过原入口启动。
