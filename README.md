# Codex 时区启动器 · Linux

为 Codex 设置启动时区，沿用原有登录和用户数据。

基于 [上游 v0.2.1](https://github.com/iUshio/codexTimeZoneRev/tree/v0.2.1)，Linux 版当前发布 **v0.1**。

## 功能

- 选择地区时区或 UTC 固定偏移，预览目标时间。
- 自动识别客户端，保存启动设置。
- 沿用原桌面入口的代理、输入法和登录环境。

## 安装

要求 **Linux x86_64、glibc ≥ 2.39**，并已安装 Codex 桌面客户端。已在 Ubuntu/KDE 实测。

从 [Releases](https://github.com/FlandreBlood/codexTimeZoneRev/releases/latest) 下载 `.deb`：

```bash
sudo apt install ./CodexTimeZone-Linux-amd64-v0.1.deb
codex-timezone
```

也可从应用菜单打开 **Codex 时区启动器 (Linux)**。免安装包和校验文件见 Releases。

## 使用

完全退出 Codex，选择时区，点击 **保存并启动**。仅影响新启动的客户端，系统时区保持不变。

关闭后从应用菜单或 `codex-timezone` 重新打开。

## 文档

- [安装与发布说明](docs/Linux-v0.1发布说明.md)
- [源码构建与运行](docs/Linux运行说明.md)
- [登录复用说明](docs/Linux登录状态修复.md)
- [安全审计](docs/安全审计与运行边界.md)

## 制作者

- 原作者：[iUshio](https://github.com/iUshio)
- Linux 适配与维护：[FlandreBlood](https://github.com/FlandreBlood)
- AI 辅助开发：Codex（OpenAI）

项目沿用上游 [LICENSE](LICENSE)。
