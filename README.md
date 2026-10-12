# AIGate CLI

通过命令行管理云扉 AIGate GPU 资源与实例。

AIGate CLI 面向个人账户，基于云扉 OpenAPI 提供资源查询、实例创建、开关机与释放，支持系统凭证存储、JSON 输出和 shell 自动补全。

[快速开始](#快速开始) · [安装与构建](#构建与运行) · [命令指南](#命令指南) · [自动补全](#shell-自动补全) · [Agent 使用](#agent-使用) · [文档](#文档与开发) · [TODO](#todo)

## 特性

- **资源查询**：查看区域、GPU 规格、个人与社区镜像、实例详情和状态，支持完整分页。
- **实例管理**：创建、开机、关机与释放；创建和释放提供离线预览与明确确认。
- **凭证存储**：通过系统 Keyring 保存 Token，也可使用进程环境变量认证。
- **自动化输出**：提供 JSON、稳定的退出码和默认脱敏，便于脚本及 Agent 调用。
- **Shell 集成**：支持 Bash、Zsh、Fish 的离线补全，由用户自行配置加载。

目前已在 **Arch Linux x86_64** 验证，并通过 **Ubuntu 24.04 x86_64** 的 CI 构建与自动测试。Ubuntu 桌面凭证、macOS 与 Windows 的实机功能仍待验收，详见 [平台支持](docs/platforms.md)。

## 快速开始

以下示例适用于 Linux 的 Bash、Zsh 和 Fish。在仓库根目录执行，先准备 [构建依赖](#构建与运行)，并确保安装目录已加入 PATH。

### 1. 安装

```bash
git clone https://github.com/davisye/aigate-cli.git
cd aigate-cli
make install
aigate-cli --version
```

默认安装到当前用户的 `~/.local/bin`，无需 `sudo`。PATH 配置和其他安装方式见下文。

### 2. 登录

在 [云扉控制台](https://waas.aigate.cc) 创建 Access Token，然后在当前用户的桌面终端执行：

```bash
env -u AIGATE_TOKEN aigate-cli login
```

按提示粘贴 Token 并回车，输入不会回显。Token 保存到系统 Keyring 的 `aigate-cli/default`；如有解锁提示，先解锁密钥环。

### 3. 查询

```bash
env -u AIGATE_TOKEN aigate-cli skus
env -u AIGATE_TOKEN aigate-cli images --all
env -u AIGATE_TOKEN aigate-cli list --all --json
```

`env -u AIGATE_TOKEN` 让本次命令使用 Keyring。空镜像或实例列表也可以是有效结果。下一步可 [配置补全](#shell-自动补全)，或查看 [命令指南](#命令指南)。

## 构建与运行

需要 Rustup；仓库通过 `rust-toolchain.toml` 固定 Rust 1.97.1，Rustup 会在需要时安装该工具链。最低 Rust 版本尚未验证。Linux 源码构建需要 `pkg-config` 和 D-Bus 开发文件，Keyring 需要可用的 Secret Service（例如 GNOME Keyring）。使用 Makefile 还需要 GNU Make 和 GNU `install`。其他平台要求见 [平台说明](docs/platforms.md#构建与终端差异)。

v0.1.0 正在准备，计划先提供源码包。尚未提供预编译二进制；Workflow API 和 Comfy API 的后续计划见 [TODO](#todo)。

### 源码安装

以下命令均在仓库根目录执行，选择一种安装方式：

| 方式 | 命令 | 默认安装位置 |
| --- | --- | --- |
| Linux 个人安装 | `make install` | `~/.local/bin/aigate-cli` |
| Linux 全局安装 | `sudo make install` | `/usr/local/bin/aigate-cli` |
| Cargo 安装 | `cargo install --path . --locked` | 用户 Cargo 目录下的 `bin` |

Make 安装会先编译 release 版本。通过 sudo 执行时，构建阶段使用原登录用户的 Rust 工具链，安装阶段由 root 写入目标目录。安装后以普通用户运行 CLI，访问该用户自己的 Keyring。

Cargo 安装是各平台的源码安装入口；macOS / Windows 的验收状态见 [平台支持](docs/platforms.md)。Cargo 安装位置也受 `--root`、`CARGO_INSTALL_ROOT` 和配置影响，详见 [Cargo 文档](https://doc.rust-lang.org/cargo/commands/cargo-install.html)。

若个人安装后找不到程序，将安装目录加入 PATH：

| Shell | 配置方式 |
| --- | --- |
| Bash / Zsh | 在对应配置文件中添加 `export PATH="$HOME/.local/bin:$PATH"` |
| Fish | 执行 `fish_add_path "$HOME/.local/bin"` |

用 `command -v aigate-cli` 检查实际调用位置；原生 PowerShell 使用 `Get-Command aigate-cli`。存在多份安装时，PATH 顺序决定调用哪一份；Cargo 安装则应将其 `bin` 目录加入 PATH。

### 构建、更新与卸载

| 操作 | 命令 |
| --- | --- |
| 构建 release | `cargo build --release --locked`；Linux 也可用 `make build` |
| 从源码运行 | `cargo run --locked -- --help` |
| 更新 | 重新执行所选安装命令 |
| 卸载 Make 个人安装 | `make uninstall` |
| 卸载 Make 全局安装 | `sudo make uninstall` |
| 卸载 Cargo 安装 | `cargo uninstall aigate-cli` |
| 清理构建产物 | `cargo clean`；Linux 也可用 `make clean` |

默认构建目录为 `target/`，release 程序为 `target/release/aigate-cli`（Windows 带 `.exe`）。依赖缓存使用 Cargo 默认目录，可通过 `CARGO_HOME`、`CARGO_TARGET_DIR` 覆盖。清理保留依赖下载缓存、已安装程序和凭证；卸载仅移除程序，删除已保存凭证须运行 `aigate-cli logout`。

Make 支持 `PREFIX`、`BINDIR`、`DESTDIR`，例如 `make install PREFIX=/usr/local DESTDIR="$PWD/staging"` 可暂存安装。`make help` 列出全部目标。

## 认证

凭证按以下顺序解析：

1. 进程环境变量 `AIGATE_TOKEN`。
2. 系统 Keyring 的 `aigate-cli/default`。

已设置但为空或无效的环境变量会报错，不回退到 Keyring。CI、服务器或无桌面环境可由秘密管理器临时注入 `AIGATE_TOKEN`；不要将 Token 写入代码、shell 配置或全局环境。CLI 没有 `--token` 参数。

`aigate-cli login` 接受纯 Token 或 `Bearer <Token>`，仅保存凭证；使用 `list --all` 等只读查询验证认证。再次登录会更新同一项。`login` 支持隐藏终端输入或标准输入，已有 Keyring 登录可继续复用。

`aigate-cli logout` 删除本地 Keyring 项。若仍设置环境变量，Bash / Zsh 用 `unset AIGATE_TOKEN` 清除，Fish 用 `set -e AIGATE_TOKEN`；PowerShell 语法见 [平台说明](docs/platforms.md#构建与终端差异)。

### 凭证与安全边界

- Keyring 凭证仅绑定官方 origin `https://waas.aigate.cc`，比较时归一化域名大小写和默认端口。自定义 origin 必须显式提供对应环境凭证，`login/logout` 只管理官方凭证。
- Linux 使用 Secret Service；macOS Keychain 和 Windows Credential Manager 后端已配置但未完成实机验收。凭证库不可用时不降级为明文文件，后端对应关系见 [keyring 文档](https://docs.rs/keyring/3.6.3/keyring/)。
- 原生凭证库不能作为同一用户进程之间的可靠应用隔离，服务名和账户名也不是授权机制。见 [GNOME 安全说明](https://wiki.gnome.org/Projects/GnomeKeyring/SecurityFAQ) 与 [Windows 凭证说明](https://learn.microsoft.com/en-us/windows/win32/secauthn/kinds-of-credentials)。
- `logout` 不撤销服务端 Token；交互确认、`--yes` 和 Agent 技能属于调用侧约束，不能代替服务端权限、过期与撤销。

## 命令指南

`aigate-cli --help` 查看全部命令，`aigate-cli COMMAND --help` 查看具体选项。下文 `INSTANCE_ID`、`IMAGE_ID`、`GPU_SKU` 为占位符，执行前替换为实际查询结果；ID 须保留完整正整数文本。

### 资源查询

| 目的 | 示例 |
| --- | --- |
| 区域列表（离线） | `aigate-cli regions` |
| 两个区域的 GPU 规格 | `aigate-cli skus` |
| 指定区域规格 | `aigate-cli skus --area 华东一区` |
| 全部个人镜像 | `aigate-cli images --all` |
| 全部计费中实例 | `aigate-cli list --all` |
| 运行中实例 | `aigate-cli list --operation-status 2 --all` |
| 已释放实例 | `aigate-cli list --operation-status 4 --all` |
| 实例详情 / 状态 | `aigate-cli info INSTANCE_ID` / `aigate-cli status INSTANCE_ID` |

社区镜像查询须提供区域、SKU 与非空名称，版本可用 `--image-version` 过滤：

```bash
aigate-cli images --image-type 2 --area 华东一区 --sku GPU_SKU --name ComfyUI --all
```

`gpus`、`stock` 是 `skus` 的别名。规格列表不保证当前有可分配库存，价格和容量按 API 原值展示。

`list`、`images` 默认只返回一页（20 条），`--all` 获取全部页，与显式 `--page` 互斥。实例页大小为 1–20，镜像为 5–20。默认实例列表按 API 返回计费中实例，已释放记录需显式筛选 `4`。

当前支持社区镜像 `2`、个人镜像 `3`。系统镜像 `1` 不受当前实现支持；需要官方环境时，可先在控制台保存为个人镜像，再查询 ID 用于创建。接口依据和验证边界见 [开发文档](docs/development.md#已知行为与验证边界)。

详情默认隐藏密码、Token、工具配置和连接 URL 查询串。确需查看连接凭证时，显式使用 `aigate-cli info INSTANCE_ID --show-secrets`。

### 创建实例

先离线预览请求，无需登录，也不访问 API：

```bash
aigate-cli create --sku GPU_SKU --area 华东一区 --image-id IMAGE_ID --image-type 3 --count 1 --dry-run
```

核对后移除 `--dry-run`，在终端输入 `create` 确认。创建会启动新实例，可能立即计费；脚本执行须在已确认具体参数后显式使用 `--yes`（`-y`）。

`--sku` 与 `--gpu-model` 二选一；区域、镜像 ID 和类型须明确指定。GPU 数量为 1–8，默认 1。`--yes` 与 `--dry-run` 互斥。

### 开关机与释放

```bash
aigate-cli stop INSTANCE_ID
aigate-cli start INSTANCE_ID
aigate-cli status INSTANCE_ID
```

关机仅允许状态 `2`，开机仅允许已有实例状态 `22` 或 `26`。未知状态保留原始码并拒绝开关机。请求被接受后，用 `status` 核对最终状态。

释放须单独确认目标 ID：

```bash
aigate-cli release INSTANCE_ID --dry-run
aigate-cli release INSTANCE_ID
```

**释放会删除实例资源，不能再开机。** 执行前备份所需数据；交互确认须输入完整实例 ID。脚本须明确使用 `--yes`，`rm` 是 `release` 的别名。暂停并保留实例使用 `stop`。

### JSON 与网络选项

```bash
aigate-cli list --all --json
aigate-cli status INSTANCE_ID --json
aigate-cli --no-proxy skus
aigate-cli --timeout 60 info INSTANCE_ID
```

`--json` 可放在子命令前后，输出也默认脱敏。分页为 `{ "total", "records" }`，SKU 为数组，详情 / 创建为 API 的 `data`；状态包含 `instanceId`、`status`、`label`，开关机 / 释放包含 `action`、`instanceId`、`accepted`。提示写入 stderr；`login/logout` 输出文字，`init` 始终输出 shell 脚本。

| 选项 | 行为 |
| --- | --- |
| `--base-url` | 覆盖 API origin，默认 `https://waas.aigate.cc`；只接受 HTTPS，HTTP loopback 用于本地 mock；禁止路径、查询串和用户信息 |
| `--timeout` | 单次请求超时 1–300 秒，默认 30 秒 |
| `--no-proxy` | 忽略代理环境变量；默认尊重系统代理设置 |

所有请求禁止自动重试和重定向。创建、开关机或释放出现超时或响应异常时，先查状态、列表或控制台，避免重复提交。`accepted: true` 只表示请求已接受，不代表生命周期已完成。

退出码：`0` 成功或预览成功；`1` API、网络、凭证或确认失败；`2` 参数错误。

## Shell 自动补全

用户自行将对应初始化行添加到 shell 配置，安装程序不会自动写入补全文件或修改配置。初始化与 Tab 补全均离线运行，不读取凭证或请求 API。

**Bash** — `~/.bashrc`：

```bash
eval "$(aigate-cli init bash)"
```

**Zsh** — `~/.zshrc`，放在 `compinit` 或 shell 框架初始化之后：

```zsh
eval "$(aigate-cli init zsh)"
```

尚未启用 Zsh 补全时，先添加 `autoload -Uz compinit` 和 `compinit`。

**Fish** — `~/.config/fish/config.fish`（自定义 `XDG_CONFIG_HOME` 时使用对应路径）：

```fish
if status is-interactive
    aigate-cli init fish | source
end
```

重新打开终端后按 Tab 可补全命令、别名、选项、区域、镜像类型、筛选状态和 shell 名称。实例 ID、镜像 ID、SKU 需先查询；尚未实现 PowerShell 补全。

## Agent 使用

仓库内提供 [aigate-cli skill](skills/aigate-cli/SKILL.md)，说明命令选择、认证复用、授权确认和失败处理。可让 Agent 先读取它：

```text
请读取本仓库的 skills/aigate-cli/SKILL.md，使用 aigate-cli 查询我的实例与可用 GPU 规格。
```

技能不会自动安装到全局目录。创建、开关机、释放和凭证修改仍按当前用户授权执行。

## 文档与开发

| 文档 | 内容 |
| --- | --- |
| [开发与测试](docs/development.md) | 代码结构、API 映射、自动测试、手动验收与已知限制 |
| [平台支持](docs/platforms.md) | 平台状态、原生依赖、终端差异与二进制兼容性 |
| [Agent skill](skills/aigate-cli/SKILL.md) | Agent 调用工具的操作流程 |
| [第三方来源与许可](THIRD_PARTY_NOTICES.md) | 参考项目与依赖许可说明 |
| [贡献指南](CONTRIBUTING.md) | 问题反馈、修改与验证要求 |
| [安全政策](SECURITY.md) | 安全问题的私密报告方式 |
| [更新记录](CHANGELOG.md) | 未发布变化与发行记录 |

Linux 使用 `make check` 运行格式、clippy、Rust 测试、安装及发行脚本回归；测试还需 Git、jq，完整补全检查需 Bash、Zsh、Fish。其他平台的 Cargo 检查见开发文档。自动测试使用本地 HTTP mock 和占位凭证，不操作真实云资源。

## TODO

- [ ] 支持 Workflow API。
- [ ] 支持 Comfy API（ComfyUI）。

## 许可证

本项目采用 [Apache License 2.0](LICENSE)。第三方依赖遵循各自许可证，详见 [第三方来源与许可](THIRD_PARTY_NOTICES.md)。
