# 平台支持

核对日期：2026-10-15。原生后端配置与实际平台验收分别记录。

## 支持状态

| 平台 | 状态 | 凭证后端 |
| --- | --- | --- |
| Arch Linux x86_64，GNU 环境 | 已验证：Rust 1.97.1、自动测试、安装回归、release 构建及 GNOME Keyring；真实 API 验证边界见 [开发文档](development.md#已知行为与验证边界) | Secret Service |
| Ubuntu 24.04 x86_64 | CI 已验证：固定工具链、自动测试、安装回归、Bash/Zsh/Fish 补全及 release 构建；桌面凭证库与真实 API 待验证 | Secret Service |
| macOS Apple Silicon | 原生 CI 已验证：固定工具链、格式、clippy、本地 mock 与 shell 补全测试；桌面凭证库和真实 API 待验证 | Keychain |
| Windows x64 MSVC | 原生 CI 已验证：固定工具链、格式、clippy、本地 mock 测试；桌面凭证库和真实 API 待验证 | Credential Manager |
| Intel Mac、其他架构、musl | 尚未验证，按需求补充 | 以对应目标实际后端为准 |

配置见 [Cargo.toml](../Cargo.toml)，后端映射见 [keyring 文档](https://docs.rs/keyring/3.6.3/keyring/)。当前最低 Rust 版本未验证，因此未声明 `rust-version`。

仓库 CI 包含 Ubuntu 24.04 x86_64、Windows 2025 x64 和 macOS 15 Apple Silicon，使用 `rust-toolchain.toml` 固定的工具链。2026-10-15 首次三平台 [原生检查](https://github.com/davisye/aigate-cli/actions/runs/38699756721) 均通过；同次依赖审计发现 rustls 安全通告，另以补丁升级修复。2026-10-12 首次远端 [Linux checks](https://github.com/davisye/aigate-cli/actions/runs/38239804112/job/114779334603) 已通过。CI 的本地 mock 测试不包含桌面凭证库和真实云资源验收，不能据此将其他平台标记为全部通过。

## 构建与终端差异

源码安装入口为 `cargo install --path . --locked`，缓存、安装目录和卸载说明见 [README](../README.md#构建与运行)。平台要求如下：

- Linux：构建需要 `pkg-config` 和 D-Bus 开发文件；原生凭证需要可访问且已解锁的 Secret Service。Makefile 依赖 Linux 工具及 GNU `install`，提供个人和全局安装。
- macOS：使用原生 Rust 工具链和系统开发工具，经 Cargo 构建；Makefile 的 GNU 工具参数尚未适配原生 macOS。
- Windows：使用 MSVC Rust 工具链及 C++ 构建工具，在原生 PowerShell 中运行 Cargo；产物带 `.exe`。工具链要求见 [Rustup 安装说明](https://rust-lang.github.io/rustup/installation/windows-msvc.html)。

WSL 属于 Linux 环境。Git Bash 与原生 PowerShell 分别验收，跨目标编译成功也不能代替目标系统运行验证。补全目前仅支持 Bash、Zsh、Fish，尚未实现 PowerShell 补全。

Linux/macOS 可用 `env -u AIGATE_TOKEN aigate-cli ...` 排除算力 / Comfy 的环境凭证覆盖；Workflow 对应 `AIGATE_WORKFLOW_TOKEN`。PowerShell 先执行 `Remove-Item Env:AIGATE_TOKEN -ErrorAction SilentlyContinue`，Workflow 将变量名换为 `AIGATE_WORKFLOW_TOKEN`，再执行 CLI；该移除只作用于当前会话，不读取或打印原值。

## 原生环境验收

自动测试及通用手动步骤统一见 [开发与测试](development.md#自动测试)。每个平台记录操作系统版本、架构、Rust 版本和实际执行 / 跳过的检查，另外验收：

- 原生凭证的写入、读取、更新、删除、锁定或不可用行为。使用独立测试条目；算力 / Comfy 使用 `aigate-cli/default`，Workflow 使用 `aigate-cli/workflow`，完整登录 / 删除测试应在独立操作系统账户中进行，保留日常登录。Workflow 新条目的原生读写仍待验收。
- 交互式隐藏输入、中文显示、CRLF / EOF、管道与 JSON 重定向。
- 仓库、构建目录和安装路径包含空格、中文时的行为。
- 所支持 shell 的脚本加载与交互式 Tab；非 Unix 不编译 shell 执行测试，不能报告为已执行通过。

凭证库不可用时应报错，不回退到明文配置；官方凭证不得发送到自定义 origin。CI 使用本地 mock 和占位凭证；真实云资源验收另按用户授权执行。

## 二进制兼容性

发布产物按操作系统与架构区分。Linux 需注明并测试发行版与 glibc 基线，Arch 构建不代表兼容所有 Linux。musl 或静态构建须单独验证凭证后端与 D-Bus；发布检查见 [开发文档](development.md#发布前核对)。

当前 v0.1.0 正式版仅准备源码发行，不上传本机 Arch 编译产物。独立的 nightly 工作流执行以下原生构建与离线运行检查；实际产物及对应提交见 [nightly 发行记录](https://github.com/davisye/aigate-cli/releases/tag/nightly)：

| 目标 | Runner / 构建基线 | 打包与运行检查 |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Ubuntu 24.04，glibc 2.39 | tar.gz；原生与解压后的 `--version`，原生 `--help` / `regions --json`；需要 D-Bus 运行时库 |
| `x86_64-pc-windows-msvc` | Windows 2025 x64，MSVC | ZIP；原生与解压后的 `--version`，原生 `--help` / `regions --json` |
| `aarch64-apple-darwin` | macOS 15 Apple Silicon | tar.gz；原生与解压后的 `--version`，原生 `--help` / `regions --json` |

未声明兼容更早的 glibc、macOS 或 Windows 版本；需在目标系统补充验收。二进制未代码签名或 Apple 公证。各包附带 `LICENSE`、`THIRD_PARTY_NOTICES.md`、README、CHANGELOG 与构建版本 / 提交 / 目标信息。Nightly 的下载与仓库设置见 [README](../README.md#nightly-预编译版本) 和 [开发文档](development.md#nightly-自动发行)。
