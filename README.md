# AIGate CLI

通过命令行管理云扉 AIGate GPU 资源、实例与生成任务。

AIGate CLI 面向个人账户，支持算力 OpenAPI、Workflow API 和 Comfy API，提供系统凭证存储、JSON 输出和 shell 自动补全。

[快速开始](#快速开始) · [安装与构建](#构建与运行) · [Nightly 下载](#nightly-预编译版本) · [命令指南](#命令指南) · [CI 与自动化](#ci-与自动化) · [自动补全](#shell-自动补全) · [Agent 使用](#agent-使用) · [文档](#文档与开发) · [TODO](#todo)

## 特性

- **资源查询**：查看区域、GPU 规格、个人与社区镜像、实例详情和状态，支持完整分页。
- **实例管理**：创建、开机、关机与释放；创建和释放提供离线预览与明确确认。
- **生成任务**：Workflow 与 Comfy 的任务提交、状态和结果查询、文件上传；提交前支持离线预览与明确确认。
- **凭证存储**：通过系统 Keyring 保存 Token，也可使用进程环境变量认证。
- **自动化输出**：提供 JSON、稳定的退出码和默认脱敏，便于脚本及 Agent 调用。
- **Shell 集成**：支持 Bash、Zsh、Fish 的离线补全，由用户自行配置加载。

目前已在 **Arch Linux x86_64** 验证，并通过 **Ubuntu 24.04 x86_64** 的 CI 构建与自动测试。Ubuntu 桌面凭证、macOS 与 Windows 的实机功能仍待验收，详见 [平台支持](docs/platforms.md)。

## 快速开始

以下示例面向个人桌面环境的日常使用，适用于 Linux 的 Bash、Zsh 和 Fish。登录后使用系统 Keyring，无需设置环境变量。在仓库根目录执行，先准备 [构建依赖](#构建与运行)，并确保安装目录已加入 PATH。

### 1. 安装

```bash
git clone https://github.com/davisye/aigate-cli.git
cd aigate-cli
cargo install --path . --locked
aigate-cli --version
```

Cargo 默认安装到当前用户的 `~/.cargo/bin`（Windows 为 `%USERPROFILE%\.cargo\bin`），无需管理员权限；原生 PowerShell 也使用上述 Cargo 命令。PATH 配置和其他安装方式见下文。

### 2. 登录

在 [云扉控制台](https://waas.aigate.cc) 创建 Access Token，然后在当前用户的桌面终端执行：

```bash
aigate-cli login
```

按提示粘贴 Token 并回车，输入不会回显。Token 保存到系统 Keyring 的 `aigate-cli/default`；如有解锁提示，先解锁密钥环。

### 3. 查询

```bash
aigate-cli skus
aigate-cli images --all
aigate-cli status
aigate-cli list --all --json
```

后续查询直接复用已保存的登录凭证。空镜像或实例列表也可以是有效结果。下一步可 [配置补全](#shell-自动补全)，或查看 [命令指南](#命令指南)。无人值守任务见 [CI 与自动化](#ci-与自动化)。

## 构建与运行

需要 Rustup；仓库通过 `rust-toolchain.toml` 固定 Rust 1.97.1，Rustup 会在需要时安装该工具链。最低 Rust 版本尚未验证。Linux 源码构建需要 `pkg-config` 和 D-Bus 开发文件，Keyring 需要可用的 Secret Service（例如 GNOME Keyring）。使用 Makefile 还需要 GNU Make 和 GNU `install`。其他平台要求见 [平台说明](docs/platforms.md#构建与终端差异)。

本次预发行版本为 `v0.1.0-beta.1`，正式版 `v0.1.0` 尚未发布。Beta 与 nightly 提供四个平台的预编译包。本地功能分支中的生成 API 已完成 mock 验证，尚未纳入本次 beta；真实任务与上传仍待验收，详见 [开发文档](docs/development.md#生成-api-验证边界)。

### Beta 预发行版

对应 tag 的全部测试、审计和打包检查通过后，可从 [v0.1.0-beta.1](https://github.com/davisye/aigate-cli/releases/tag/v0.1.0-beta.1) 下载带版本号的附件：

| 平台 | 下载 |
| --- | --- |
| Linux aarch64（ARM64） | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/v0.1.0-beta.1/aigate-cli-0.1.0-beta.1-aarch64-unknown-linux-gnu.tar.gz) |
| Linux x86_64 | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/v0.1.0-beta.1/aigate-cli-0.1.0-beta.1-x86_64-unknown-linux-gnu.tar.gz) |
| Windows x64 | [ZIP](https://github.com/davisye/aigate-cli/releases/download/v0.1.0-beta.1/aigate-cli-0.1.0-beta.1-x86_64-pc-windows-msvc.zip) |
| macOS Apple Silicon | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/v0.1.0-beta.1/aigate-cli-0.1.0-beta.1-aarch64-apple-darwin.tar.gz) |
| 校验和 | [SHA256SUMS](https://github.com/davisye/aigate-cli/releases/download/v0.1.0-beta.1/SHA256SUMS) |

解压后使用 `./aigate-cli --version`（Windows 为 `./aigate-cli.exe --version`）核对 `0.1.0-beta.1`。运行依赖、PATH 与校验方法见下文，平台基线见 [平台说明](docs/platforms.md)。Beta 标签和附件不覆盖更新；后续修复发布新的 beta 编号。

### Nightly 预编译版本

Nightly 配置合入 `main`、维护者完成 [发行设置](docs/development.md#nightly-自动发行)，且工作流首次成功后，可从固定的 [nightly 预发行版](https://github.com/davisye/aigate-cli/releases/tag/nightly) 下载，无需安装 Rust 或 Make：

| 平台 | 固定下载地址 |
| --- | --- |
| Windows x64（MSVC） | [ZIP](https://github.com/davisye/aigate-cli/releases/download/nightly/aigate-cli-nightly-x86_64-pc-windows-msvc.zip) |
| Linux x86_64（Ubuntu 24.04 构建基线） | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/nightly/aigate-cli-nightly-x86_64-unknown-linux-gnu.tar.gz) |
| Linux aarch64（Ubuntu 24.04 ARM64 构建基线） | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/nightly/aigate-cli-nightly-aarch64-unknown-linux-gnu.tar.gz) |
| macOS Apple Silicon（macOS 15 构建基线） | [tar.gz](https://github.com/davisye/aigate-cli/releases/download/nightly/aigate-cli-nightly-aarch64-apple-darwin.tar.gz) |
| 校验和 | [SHA256SUMS](https://github.com/davisye/aigate-cli/releases/download/nightly/SHA256SUMS) |

Windows 解压 ZIP，在 PowerShell 中先运行 `./aigate-cli.exe --version`，再将程序所在目录加入用户 PATH。可用 `Get-FileHash ./aigate-cli-nightly-x86_64-pc-windows-msvc.zip -Algorithm SHA256` 与同次下载的 `SHA256SUMS` 比较。

Linux / macOS 解压后运行 `./aigate-cli --version`，再放入 PATH 中的用户目录。Linux 需要运行时 D-Bus 库（Ubuntu 包名 `libdbus-1-3`）；桌面登录仍需 Secret Service。包内含许可、文档与 `BUILD-INFO.txt`。二进制未签名或公证；平台功能验收边界见 [平台说明](docs/platforms.md)。

固定链接会随成功构建更新；`--version` 显示基础版本、nightly UTC 日期及提交号。Nightly 标记为 prerelease，不作为正式版 Latest。每天北京时间 03:23 调度，未变化的提交跳过发布；GitHub 调度可能延迟，手动运行可重新构建同一提交。

### 源码安装

以下命令均在仓库根目录执行，选择一种安装方式：

| 方式 | 命令 | 默认安装位置 |
| --- | --- | --- |
| Cargo 安装（各平台首选） | `cargo install --path . --locked` | 用户 Cargo 目录下的 `bin` |
| Linux 个人安装 | `make install` | `~/.local/bin/aigate-cli` |
| Linux 全局安装 | `sudo make install` | `/usr/local/bin/aigate-cli` |

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

默认构建目录为 `target/`，release 程序为 `target/release/aigate-cli`（Windows 带 `.exe`）。依赖缓存使用 Cargo 默认目录，可通过 `CARGO_HOME`、`CARGO_TARGET_DIR` 覆盖。清理保留依赖下载缓存、已安装程序和凭证；卸载仅移除程序。按需用 `aigate-cli logout`、`aigate-cli workflow logout` 删除对应凭证。

Make 支持 `PREFIX`、`BINDIR`、`DESTDIR`，例如 `make install PREFIX=/usr/local DESTDIR="$PWD/staging"` 可暂存安装。`make help` 列出全部目标。

## 认证

### 日常使用：系统 Keyring

在自己的桌面终端执行一次 `aigate-cli login`，后续直接运行查询和实例管理命令。Token 保存在系统 Keyring，无需设置 `AIGATE_TOKEN`。

`login` 接受纯 Token 或 `Bearer <Token>`，隐藏输入并保存凭证；使用 `status` 或 `list --all` 等只读查询验证认证。再次登录会更新同一项，已有 Keyring 登录可继续复用。`login` 也支持从标准输入读取，默认交互方式更适合个人使用。

Workflow 使用 Studio 单独签发的 Token，执行 `aigate-cli workflow login` 保存。Comfy 复用现有算力登录。两套凭证相互独立：

| 服务 | 登录 / 退出 | 官方 origin | Keyring 项 | 可选环境变量 |
| --- | --- | --- | --- | --- |
| 算力与 Comfy | `aigate-cli login` / `logout` | `https://waas.aigate.cc` | `aigate-cli/default` | `AIGATE_TOKEN` |
| Workflow | `aigate-cli workflow login` / `workflow logout` | `https://studio.aigate.cc` | `aigate-cli/workflow` | `AIGATE_WORKFLOW_TOKEN` |

### 凭证优先级与排查

每套服务的凭证按以下顺序解析：

1. 表中对应的进程环境变量。
2. 表中对应的系统 Keyring 项。

环境变量适用于 [CI 与自动化](#ci-与自动化)，会覆盖对应的 Keyring 项；已设置但为空或无效时会报错，不回退到 Keyring，也不回退到另一套服务的 Token。CLI 没有 `--token` 参数。

如果曾设置环境变量，恢复 Keyring 认证时，Bash / Zsh 用 `unset AIGATE_TOKEN AIGATE_WORKFLOW_TOKEN`，Fish 用 `set -e AIGATE_TOKEN; set -e AIGATE_WORKFLOW_TOKEN`；PowerShell 语法见 [平台说明](docs/platforms.md#构建与终端差异)。退出登录只处理对应的 Keyring 项，环境变量须在调用侧清除。

### 凭证与安全边界

- 每套 Keyring 凭证仅绑定表中对应的官方 origin，比较时归一化域名大小写和默认端口。自定义 origin 必须显式提供对应环境凭证，登录与退出只管理官方凭证。
- Linux 使用 Secret Service；macOS Keychain 和 Windows Credential Manager 后端已配置但未完成实机验收。凭证库不可用时不降级为明文文件，后端对应关系见 [keyring 文档](https://docs.rs/keyring/3.6.3/keyring/)。
- 原生凭证库不能作为同一用户进程之间的可靠应用隔离，服务名和账户名也不是授权机制。见 [GNOME 安全说明](https://wiki.gnome.org/Projects/GnomeKeyring/SecurityFAQ) 与 [Windows 凭证说明](https://learn.microsoft.com/en-us/windows/win32/secauthn/kinds-of-credentials)。
- `logout` 不撤销服务端 Token；交互确认、`--yes` 和 Agent 技能属于调用侧约束，不能代替服务端权限、过期与撤销。
- Token 通过系统 Keyring 保存或由秘密管理器临时注入；不写入源码、shell 配置、工作流明文或全局环境。

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
| 计费中实例状态总览 | `aigate-cli status` |
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

### 状态总览

```bash
aigate-cli status
aigate-cli status --json
aigate-cli status INSTANCE_ID
```

不带 ID 时自动读取当前账户计费中实例的全部分页，显示总数、各状态数量，以及实例 ID、名称、状态、区域和 GPU 数量。未知状态保留原始码；没有实例时明确显示空结果。已释放历史记录使用 `list --operation-status 4 --all` 查询。

`status --json` 返回 `{ "total", "summary", "records" }`。`summary` 按状态码汇总，每项包含 `status`、`label`、`count`；`records` 保留列表字段并增加状态说明 `label`，默认脱敏。汇总来自列表中的 `operationStatus`；开关机等操作完成后，使用 `status INSTANCE_ID` 单独核对最终状态。

分页不完整、状态字段无效或 API 失败时返回非零退出码，不输出部分总览。

### 创建实例

先离线预览请求，无需登录，也不访问 API：

```bash
aigate-cli create --sku GPU_SKU --area 华东一区 --image-id IMAGE_ID --image-type 3 --count 1 --dry-run
```

核对后移除 `--dry-run`，在终端输入 `create` 确认。创建会启动新实例，可能立即计费；脚本执行须在已确认具体参数后显式使用 `--yes`（`-y`）。

`--sku` 与 `--gpu-model` 二选一；区域、镜像 ID 和类型须明确指定。GPU 数量为 1–8，默认 1。`--yes` 与 `--dry-run` 互斥。

>
> 当前云菲 aigete 没有提供官方系统镜像ID获取API。所以创建实例需要先手动创建容器并保存为个人镜像，或者使用社区镜像进行容器创建。
>

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

### Workflow API

前往 [Studio](https://studio.aigate.cc) 创建工作流 API Token，并从所选工作流页面取得 `APP_ID`、输入参数定义和积分消耗。工作流参数各不相同，以页面说明为准。使用方法依据 [官方 Workflow API](https://docs.aigate.cc/developer/workflowAPI)。

```bash
aigate-cli workflow login
aigate-cli workflow run --app-id APP_ID --input params.json --dry-run
aigate-cli workflow run --app-id APP_ID --input params.json
aigate-cli workflow status TASK_ID
```

`params.json` 只包含 `inputParams` 内的 JSON object，CLI 会添加外层 `appId` 与 `inputParams`。例如以下参数仅适用于要求这些字段的工作流：

```json
{
  "ratio": "9:16",
  "pixel": "768P",
  "prompt": "A cinematic city scene at dusk.",
  "time": 5
}
```

`--dry-run` 离线预览且无需登录。实际提交须在终端输入 `submit`；已明确确认的自动化可使用 `--yes`，与 `--dry-run` 互斥。生成会消耗积分，排队任务暂不支持取消。保存返回的 `taskId`，再用 `workflow status` 查询。

需要文件输入时先上传，再按照该工作流的参数说明，把返回的文件信息写入 `params.json`：

```bash
aigate-cli workflow upload ./input.png --media-type image
aigate-cli workflow upload ./clip.mp4 --media-type video
aigate-cli workflow upload ./voice.wav --media-type audio
```

图片限 5 MB，视频和音频限 20 MB；CLI 按十进制 MB 检查。上传支持 `--dry-run` 预览文件名、大小与接口，不发送文件。

### Comfy API

Comfy API 复用 `aigate-cli login` 的算力 Token。先在控制台配置应用及绑定的 ComfyUI 实例，再查询应用、输入定义和实例。接口依据 [官方 Comfy API](https://docs.aigate.cc/developer/comfyAPI)。

```bash
aigate-cli comfy apps
aigate-cli comfy apps --json
aigate-cli comfy instances APP_ID
aigate-cli comfy upload ./input.png --app-id APP_ID --instance-id INSTANCE_ID
aigate-cli comfy run --app-id APP_ID --instance-id INSTANCE_ID --input inputs.json --dry-run
aigate-cli comfy run --app-id APP_ID --instance-id INSTANCE_ID --input inputs.json
aigate-cli comfy status TASK_ID
```

`inputs.json` 只包含应用 `allowInput` 内的 JSON object，不是完整的 ComfyUI 工作流图。按应用的 `allowInput`、别名和类型定义填写；图片输入使用上传返回的 `name`。应用和实例 ID 须为完整正 int64，提交时按官方要求发送字符串。

任务提交的确认方式与 Workflow 相同。CLI 只向指定应用、实例提交任务，不自动创建或开机。上传会核对应用归属和运行中状态，文件流式发送到实例的 `input` 目录；实例上传不携带平台 Bearer Token，也不跟随重定向。Comfy 的 `instanceStatus` 与算力的 `operationStatus` 是不同的状态体系。

### 任务状态与生成结果

两套 `status TASK_ID` 均输出 JSON，保留 API 字段并增加 `label`。状态 `0` 为排队、`1` 为生成中、`2` 为成功、`3` 为取消、`4` 为失败、`5` 为超时；Workflow 还支持 `6` 上传中。未知状态保留原值。查询退出码 `0` 表示查询正常，任务是否成功须检查 `status == 2`。

`TASK_ID` 使用提交返回的任务标识，不能填写结果 URL。数字 ID 须为正 int64；其他标识仅支持字母、数字、下划线和连字符，须以字母或数字开头。

结果位于 `files`；暂无文件时可能为 `null` 或空数组。第一版不自动轮询或下载，可间隔查询，成功后自行保存文件。Comfy 的下载链接与实例绑定，关机或释放后可能失效。默认隐藏 URL 查询串；确需取出带签名链接时，在私密终端使用 `workflow status TASK_ID --show-secrets` 或 `comfy status TASK_ID --show-secrets`，Workflow 上传也支持该选项。

### JSON 与网络选项

```bash
aigate-cli list --all --json
aigate-cli status --json
aigate-cli status INSTANCE_ID --json
aigate-cli --no-proxy skus
aigate-cli --timeout 60 info INSTANCE_ID
```

`--json` 可放在子命令前后，输出也默认脱敏。分页为 `{ "total", "records" }`，状态总览为 `{ "total", "summary", "records" }`，SKU 为数组，详情 / 创建为 API 的 `data`；单实例状态包含 `instanceId`、`status`、`label`，开关机 / 释放包含 `action`、`instanceId`、`accepted`。提示写入 stderr；`login/logout` 输出文字，`init` 始终输出 shell 脚本。

| 选项 | 行为 |
| --- | --- |
| `--base-url` | 覆盖当前服务的 origin；Workflow 默认 `https://studio.aigate.cc`，其他命令默认 `https://waas.aigate.cc`；只接受 HTTPS，HTTP loopback 用于本地 mock；禁止路径、查询串和用户信息 |
| `--timeout` | 单次请求超时 1–300 秒，默认 30 秒 |
| `--no-proxy` | 忽略代理环境变量；默认尊重系统代理设置 |

所有请求禁止自动重试和重定向。资源操作、任务提交或上传出现超时或响应异常时，先查状态、列表或控制台记录，避免重复提交。任务提交要求有效 `taskId`，无效响应会报告结果不明；`accepted: true` 只表示实例操作请求已接受，不代表生命周期已完成。

退出码：`0` 成功或预览成功；`1` API、网络、凭证或确认失败；`2` 参数错误。

## CI 与自动化

CI runner、服务器或无桌面环境的任务，通过平台的 Secrets 或秘密管理器临时注入凭证。算力与 Comfy 使用 `AIGATE_TOKEN`，Workflow 使用 `AIGATE_WORKFLOW_TOKEN`，仅注入任务需要的变量。CLI 直接读取环境凭证，无需执行登录，也无需配置桌面 Keyring。

### GitHub Actions 示例

以下示例适用于维护者手动查询云资源：

1. 在运行任务的仓库中，进入 **Settings → Secrets and variables → Actions**，创建名为 `AIGATE_TOKEN` 的 repository secret，值为云扉 Access Token。
2. 将以下内容保存为 `.github/workflows/aigate-status.yml`，仓库中须包含本项目源码及 `rust-toolchain.toml`。
3. 将工作流提交到默认分支后，在 **Actions → AIGate status → Run workflow** 手动运行。

```yaml
name: AIGate status

on:
  workflow_dispatch:

permissions:
  contents: read

jobs:
  query:
    runs-on: ubuntu-24.04
    timeout-minutes: 15
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - name: Install build dependencies
        run: |
          sudo apt-get update
          sudo apt-get install --no-install-recommends -y pkg-config libdbus-1-dev
      - name: Build
        run: cargo build --release --locked --bin aigate-cli
      - name: Query instances
        env:
          AIGATE_TOKEN: ${{ secrets.AIGATE_TOKEN }}
        shell: bash
        run: |
          : "${AIGATE_TOKEN:?请配置 AIGATE_TOKEN repository secret}"
          ./target/release/aigate-cli status --json
          ./target/release/aigate-cli list --all --json
```

Token 仅注入查询步骤，示例输出为默认脱敏的 JSON。避免打印环境变量、启用 `set -x` 或在 CI 中使用 `--show-secrets`。Secrets 配置与手动触发方式见 [GitHub Secrets 文档](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-secrets)和[工作流运行说明](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow)。

Workflow 查询可在专门的步骤中注入同名 repository secret（`TASK_ID` 使用已知任务 ID）：

```yaml
- name: Query workflow task
  env:
    AIGATE_WORKFLOW_TOKEN: ${{ secrets.AIGATE_WORKFLOW_TOKEN }}
    TASK_ID: ${{ vars.AIGATE_WORKFLOW_TASK_ID }}
  run: ./target/release/aigate-cli workflow status "$TASK_ID" --json
```

其他 CI 平台在相应任务中注入同名环境变量，并在安装 CLI 后直接调用查询命令。自动化创建、释放或提交生成任务须先核对 `--dry-run`，获得明确授权后使用 `--yes`；查询不需要确认参数。

本仓库的开发 CI 使用本地 HTTP mock 和占位凭证，无需配置真实账户 Token。

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

重新打开终端后按 Tab 可补全命令、别名、选项、区域、镜像类型、筛选状态、上传媒体类型和 shell 名称，包括 `workflow`、`comfy` 的子命令。资源与任务 ID 需先查询或从提交结果获取；尚未实现 PowerShell 补全。

## Agent 使用

仓库内提供 [aigate-cli skill](skills/aigate-cli/SKILL.md)，说明命令选择、认证复用、授权确认和失败处理。可让 Agent 先读取它：

```text
请读取本仓库的 skills/aigate-cli/SKILL.md，使用 aigate-cli 查询我的实例与可用 GPU 规格。
```

技能不会自动安装到全局目录。云资源操作、任务提交、上传和凭证修改仍按当前用户授权执行。

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

各平台以 Cargo 运行格式、clippy 和 Rust 测试，命令见开发文档。Linux 另可用 `make check` 一并运行安装及发行脚本回归；脚本测试还需 Git、jq，完整补全检查需 Bash、Zsh、Fish。CI 配置包含 Linux、Windows 和 macOS 原生检查；已执行的平台验收记录见平台说明。自动测试使用本地 HTTP mock 和占位凭证，不操作真实云资源。

## TODO

- [x] Workflow API 第一版：独立登录、任务提交、状态与结果查询、文件上传（本地 mock 已验证）。
- [x] Comfy API 第一版：应用与实例查询、任务提交、状态与结果查询、图片上传（本地 mock 已验证）。
- [ ] 完成两套生成 API 的真实账户验收，核对官方文档未明确的响应结构。
- [ ] 增加有超时上限的任务等待与结果下载。

## 许可证

本项目采用 [Apache License 2.0](LICENSE)。第三方依赖遵循各自许可证，详见 [第三方来源与许可](THIRD_PARTY_NOTICES.md)。
