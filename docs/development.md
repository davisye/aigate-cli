# 开发与测试

用户安装、登录和命令示例见 [README](../README.md)。平台状态与原生环境要求见 [platforms.md](platforms.md)。本文维护实现约束、接口映射和验收方法；测试结果应附在对应变更或发布记录中。

## 代码结构

| 文件 | 职责 |
| --- | --- |
| `src/main.rs` | clap 命令树、参数校验、操作预览与确认、输出选择、shell 初始化 |
| `src/auth.rs` | Token 规范化、环境变量优先级、官方 origin 与 Keyring 绑定 |
| `src/client.rs` | HTTP、响应校验、分页、开关机前置状态 |
| `src/generation.rs` | Workflow / Comfy 请求、任务响应校验、文件预检与流式上传 |
| `src/task_commands.rs` | 生成 API 的命令组、参数文件、预览、提交确认与输出 |
| `src/models.rs` | 请求、分页与状态总览类型，状态码含义 |
| `src/output.rs` | 表格、JSON、递归脱敏 |

## API 映射

官方 origin 为 `https://waas.aigate.cc`，下表路径前缀为 `/api/openapi`，认证为 `Authorization: Bearer <Token>`。以 [官方 OpenAPI](https://docs.aigate.cc/developer/openAPI) 和 [认证说明](https://docs.aigate.cc/developer/apiInstruction) 为接口依据；参考项目及许可见 [第三方说明](../THIRD_PARTY_NOTICES.md)。

| CLI | 方法与路径 | 参数 |
| --- | --- | --- |
| `skus` | GET `/instance/skuList` | `areaName` |
| `list` | POST `/instance/page` | `current`, `pageSize`，可选 `operationStatus` |
| `images` | POST `/image/page` | `current`, `pageSize`, `imageType`；社区镜像增加 `areaName`, `skuName`, `imageName`, `imageVersion` |
| `info` | GET `/instance/get` | `instanceId` |
| `status`（无 ID） | POST `/instance/page` | 从 `current: 1`、`pageSize: 20` 读取全部计费中实例；按 `operationStatus` 汇总 |
| `status INSTANCE_ID` | GET `/instance/status` | `instanceId` |
| `create` | POST `/instance/start` | `skuName` 或 `gpuModel`, `areaName`, `count`, `imageId`, `imageType` |
| `start` | GET `/instance/open` | `instanceId`；预查状态 `22` / `26` |
| `stop` | GET `/instance/close` | `instanceId`；预查状态 `2` |
| `release` | GET `/instance/release` | `instanceId` |

### 生成 API 映射

依据 [Workflow 官方文档](https://docs.aigate.cc/developer/workflowAPI) 与 [Comfy 官方文档](https://docs.aigate.cc/developer/comfyAPI)。两套客户端共享平台响应校验和禁止重试、重定向的策略。

| CLI | Origin / 前缀 | 方法与末级路径 | 参数 |
| --- | --- | --- | --- |
| `workflow run` | Studio `/api/comfyui/open/v1/comfyui_workflow` | POST `/create` | `appId` 字符串，`inputParams` object |
| `workflow status` | 同上 | GET `/getTask` | query `taskId` |
| `workflow upload` | 同上 | POST `/uploadFile` | query `media_type`；multipart `file` |
| `comfy apps` | WAAS `/api/openapi/comfyui` | GET `/comfyuiApp/list` | 无 |
| `comfy instances` | 同上 | GET `/comfyuiInstance/list` | query `appId` |
| `comfy run` | 同上 | POST `/comfyuiTask/create` | `appId`、`instanceId` 字符串，`allowInput` object |
| `comfy status` | 同上 | GET `/comfyuiTask/status` | query `taskId` |
| `comfy upload` | API 返回的 `comfyuiHost` | POST `/api/upload/image` | multipart `image`、`type=input`；先查询应用绑定实例 |

Studio 为 `https://studio.aigate.cc`，WAAS 为 `https://waas.aigate.cc`。Workflow 使用 `AIGATE_WORKFLOW_TOKEN` → `aigate-cli/workflow`；算力 / Comfy 使用 `AIGATE_TOKEN` → `aigate-cli/default`。环境变量为空或无效时报错，两套凭证不互相回退。`--base-url` 按命令组选择默认 origin，自定义地址仍需显式环境凭证，不能读取另一套官方 Keyring 项。

任务输入文件只含参数 object，最多 16 MiB；CLI 添加外层字段。预览与提交确认均在凭证解析之前完成。交互输入 `submit`，非交互显式使用 `--yes`。预览和确认提示默认脱敏，实际请求保留用户参数。提交只接受有效 `taskId`，保留整数精度并在输出中规范为字符串；响应结构无效时报结果不明，不自动重放。查询核对返回任务 ID，保留原始状态并增加 `label`；退出码表示查询是否成功，不能代替任务执行状态判断。

任务 ID 只接受以字母或数字开头、仅含 ASCII 字母、数字、下划线或连字符的标识；数字 ID 须为正 int64，拒绝 URL 和 `null` 等占位值。有名字段 `data.taskId` 可为安全字符串标识，未指定 schema 的标量 `data` 仅兼容正 int64 ID，不把普通成功消息当作任务 ID。位置参数在解析后校验，错误不回显误传的签名 URL。HTTP(S) URL 脱敏的 scheme 比较不区分大小写。

上传只接受非空普通文件，文件名不得含控制字符。Workflow 按十进制 MB 检查图片 5 MB、音视频 20 MB；Comfy 文档未规定大小上限，客户端不添加该服务限制。multipart 使用流式文件体。Comfy 上传要求唯一匹配的应用和实例、`instanceStatus=2`，不自动开机；单独的 HTTP client 没有平台认证头。上传 host 支持 HTTPS、路径前缀与查询串，HTTP 仅允许本地 loopback 测试，拒绝用户信息和 fragment；诊断不回显 host 或原始网络错误。返回须含有效 `name`、`subfolder`、`type=input`。

默认隐藏 `comfyuiHost` 和结果 URL 的查询串；生成任务状态及 Workflow 上传可显式使用 `--show-secrets` 获取带签名的文件链接，不能写入共享日志。未实现自动轮询、下载、取消或应用创建。

### 生成 API 验证边界

本轮完成本地 mock、CLI 和补全验证，未调用真实生成 API、上传文件、修改登录项或消耗积分。原有算力 API 的实机验收不覆盖新功能。

官方文档存在以下空缺，后续真实验收需核对，不能把 mock 通过当作服务端兼容性确认：

- Workflow `getTask` 未给出查询参数表；当前以返回字段名 `taskId` 作为 query 参数，需实机确认。
- 两套创建接口未给出完整可用的成功响应；当前兼容 `data.taskId` 与正 int64 标量任务 ID。Comfy 的 `ok:false`、空对象占位例不会被视为成功。
- Workflow 上传未给出响应结构；当前要求标准成功 envelope，`data` 为非空字符串或 object。其 cURL 示例缺少 `/api/`，实现以“请求地址”中的完整路径为准。

真实验收时先查询 Comfy 应用及绑定实例，再由用户指定应用、参数、文件和费用范围。Workflow 单独登录；提交一次后记录 ID 并查询，结果不明先核对控制台调用记录。不自动创建、开机、关机或释放实例；保留现有登录，新增凭证仅按用户授权管理。

## 实现约束

- 创建、释放先处理离线预览或明确确认，再解析凭证、发送请求。未确认、取消或 EOF 时不得发出 HTTP 请求；`--yes` 与 `--dry-run` 互斥。
- `start` / `stop` 先查状态，再提交一次操作。状态预查不能消除并发变化；请求接受与最终状态完成须分别报告。
- 所有 HTTP 请求禁止自动重试和重定向。尤其不能按 GET 方法推断开关机、释放可安全重放。写请求结果不明时先核对列表或状态。
- 成功响应要求数值 `code: 0`；存在 `ok` 时须为 `true`。开关机与释放允许 `data: null`；创建要求有效实例 ID，查询必须验证数据形态。无效响应不能当作空结果或成功。
- ID 保留 int64 精度，不能转换为浮点数；创建请求的 `imageId` 发送数值。分页严格校验 `total`、`records` 及记录类型；到达 `total` 前出现空页须报错。
- 状态总览复用列表分页，不逐个请求状态接口。全部页成功后才输出；记录数量必须等于最终 `total`，实例 ID 须为有效且不重复的正 int64，`operationStatus` 须为非空字符串或 int64 数值。数值与对应字符串归入同一组，未知码保留原值。JSON 包含 `total`、`summary`、`records`，记录增加 `label` 并默认脱敏；终端文字输出清理控制字符。
- 原生凭证仅用于归一化后的官方 origin；自定义 origin 只接受调用者显式提供的环境凭证。URL 校验在读取凭证之前完成，无明文配置回退。
- 默认递归脱敏敏感字段、工具配置与连接 URL 查询串；日志不得暴露当前 Token。`--show-secrets` 只按显式请求显示详情凭证。安全边界见 [README](../README.md#凭证与安全边界)。

### 补全实现

`init` 从实际 clap 命令树生成脚本，在认证和 HTTP 客户端初始化之前处理；stdout 仅输出脚本，`--json` 不改变格式。区域和筛选状态的候选只加入补全树，不收紧真实参数校验，也不在线查询资源 ID。

锁定的 `clap_complete` 版本对带连字符的 Bash 程序名生成不一致的子命令路径，因此内部用 `aigate_cli` 生成，再改回注册名。Fish 生成器缺少位置参数候选，额外规则从 `CompletionShell` 枚举生成。升级依赖时检查这些兼容处理能否移除，并运行 shell 执行检查。

## 自动测试

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --release --locked --bin aigate-cli
```

Cargo 是三平台的主要构建入口，无需 Make。Linux 可用 `make check` 运行格式、clippy、Rust 测试、安装及发行脚本回归，`make build` 验证 release 构建。发行脚本回归另外需要 Git、jq、Bash 与 GNU tar / gzip / coreutils，在临时仓库中测试，不修改项目的 Git 历史。`bash tests/nightly.sh` 通过 mock GitHub CLI 验证首次发布、滚动更新、校验失败与恢复草稿，不调用真实 GitHub API。工具链由 `rust-toolchain.toml` 固定；更新工具链时重新运行全部检查。依赖和构建目录遵循 Cargo 默认值；需要隔离缓存时由执行环境覆盖 `CARGO_HOME`、`CARGO_TARGET_DIR`。

CI 配置在 Ubuntu 24.04、Windows 2025 x64、macOS 15 Apple Silicon 上运行 Cargo 格式检查、clippy 和本地 mock 测试。安装及发行脚本回归仅在 Linux 执行；Windows 不运行 Unix shell 补全测试，macOS 安装 Fish 后运行 Bash / Zsh / Fish 检查。新增工作流尚需远端运行，配置本身不代表平台验收通过。

| 测试 | 维护范围 |
| --- | --- |
| 单元测试 | Token 规范化、官方 origin 绑定、递归脱敏 |
| `tests/api.rs` | 本地 wiremock 验证方法、路径、认证、请求体、分页、ID、状态约束与总览汇总、错误与无效响应、超时、重试及重定向 |
| `tests/cli.rs` | 真实进程的参数、确认拦截、退出码、stdout/stderr、JSON、状态总览与空账户、离线命令、凭证边界 |
| `tests/generation_api.rs` | 两套任务 API 的方法、参数、状态和 ID、无效响应、上传大小与 multipart、实例选择、无认证上传、重试及重定向 |
| `tests/generation_cli.rs` | 生成命令的离线预览、提交确认、独立凭证、输入校验、退出码、JSON 与签名链接脱敏 |
| `tests/completions.rs` | Unix 上 Bash/Fish 的补全候选，Zsh 的加载与注册 |
| `tests/makefile.sh` | Linux 安装、卸载、clean、sudo 构建环境、含空格路径和构建失败；使用模拟命令与临时目录 |
| `tests/release.sh` | 临时 Git 仓库中的标签与版本、主分支归属、脏工作区、发行说明、源码包内容、校验和与重复打包一致性 |

CI 仅使用本地 mock 和占位凭证。要求完整补全检查的 Unix 环境须准备 Bash、Zsh、Fish，并运行：

```sh
env AIGATE_REQUIRE_SHELLS=1 cargo test --locked --test completions -- --nocapture
```

普通模式缺少 shell 时会跳过，`--nocapture` 显示原因；严格模式下 shell 缺失或版本命令失败均导致测试失败。非 Unix 平台不编译 shell 执行测试。记录实际执行和跳过项，不能只凭测试命令退出成功报告补全验收通过。

## 手动验收

### 终端与补全

按 [README](../README.md#shell-自动补全) 加载初始化脚本，重新打开终端，在已支持的 shell 中输入下表内容并按 Tab；无需执行资源命令。

| 光标前的输入 | 应出现的候选 |
| --- | --- |
| `aigate-cli ` | 子命令及 `gpus`、`stock`、`rm` 别名 |
| `aigate-cli create --image-` | `--image-id`、`--image-type` |
| `aigate-cli create --area 华` | `华东一区`、`华东二区` |
| `aigate-cli images --image-type ` | `2`、`3` |
| `aigate-cli list --operation-status ` | `2`、`4` |
| `aigate-cli init ` | `bash`、`zsh`、`fish` |
| `aigate-cli workflow ` | `login`、`logout`、`run`、`status`、`upload` |
| `aigate-cli workflow upload --media-type ` | `image`、`video`、`audio` |

自动测试不能替代交互式 Tab、隐藏输入和原生凭证库验收。检查终端中文、管道、CRLF / EOF、JSON 重定向以及含空格和中文的路径；原生凭证测试的隔离方式见 [平台说明](platforms.md#原生环境验收)。

### 真实账户只读冒烟

在用户要求真实测试时复用已有登录，按 [README 认证说明](../README.md#认证) 选择凭证来源。运行 `regions`、`skus`、`images --all --json`、`list --all --json`、`status`、`status --json`、`list --operation-status 4 --all --json`；核对控制台、多页数量和 `total`，空列表可为有效结果。总览各状态数量之和应等于记录总数；查询期间实例有变化时重新核对。

有实例时对查询得到的 ID 检查 `info`、`status` 及默认脱敏；没有实例则记录跳过。社区镜像按实际区域、SKU 和非空名称查询。用已查到的参数执行创建与释放的 `--dry-run`，核对离线请求；确认拦截与请求数为零由本地 mock 验证。日常冒烟保留现有 Keyring 登录。

### 经授权的生命周期验收

创建、开关机、释放分别按用户授权执行；只有明确包含释放的完整测试才以释放结束。使用专用验收实例，不能将“冒烟”自动解释为删除现有实例。

1. 核对区域、SKU / GPU 型号、镜像 ID、类型和数量；预览创建请求。明确确认后提交一次，记录返回 ID，查询至运行中 `2`。
2. 对该 ID 执行 `stop`，查询至 `22` 或 `26`；执行 `start`，查询至 `2`。设定有限等待时长，不反复提交写操作。
3. 获得该 ID 的释放确认并完成所需备份后，预览并提交一次 `release`。核对状态 `4`，已释放列表包含该 ID，计费中列表不再包含它。释放后不能重新开机。

开机可能因云端资源不足失败；超时或响应异常也可能使结果不明。先查询状态或控制台，记录未完成项，再按已有授权决定后续处理；保留个人镜像。报告请求是否接受、最后状态和跳过项，不记录 Token、连接凭证或未脱敏响应。

## 已知行为与验证边界

- 当前实现仅接受个人镜像 `3`、社区镜像 `2`；系统镜像 `1` 不受支持。需要 CLI 创建官方环境时，先由用户在控制台保存为个人镜像，再查询其 ID。已有官方镜像实例可按 ID 验收开关机。
- 社区镜像的 `imageName` 必填；部分社区示例允许空名称，真实 API 曾返回“镜像名称不能为空”。当前 CLI 在请求前校验。
- 状态码按官方含义显示并保留原始值；社区对 `3`、`22` 的显示曾有差异，未知码不得作为可开关机依据。
- 2026-10-12 在 Arch Linux 完成过查询、个人镜像创建、详情脱敏、关机和释放验收；当次重新开机因资源不足失败，用户随后反馈手动恢复成功。该记录不代表当前库存，也不代替其他平台验收；未验收 GPU 工作负载或实际扣费金额。

## 发布前核对

v0.1.0 的发行流程目前只生成源码包，不分发本机编译产物。后续二进制发行须按目标平台完成兼容性与凭证验收，附带完整第三方许可和校验和，要求见 [第三方说明](../THIRD_PARTY_NOTICES.md)。`publish = false` 仅控制 crates.io 发布；Cargo 仓库元数据应填写真实地址。

### 自动检查

- `.github/workflows/ci.yml`：PR、`main` 推送及手动触发时执行 Linux 检查与 RustSec 依赖审计。Linux 检查使用 Ubuntu 24.04、固定 Rust 工具链和严格 Bash/Zsh/Fish 补全测试；配置提交不表示远端已运行通过。
- `.github/workflows/security.yml`：每周及手动执行依赖审计，发现新公告时也能使检查失败。
- `.github/dependabot.yml`：每周检查 Cargo 和 GitHub Actions 更新。
- Actions 固定完整提交 SHA，默认 `contents: read`，checkout 不保留凭证。只有创建草稿的独立任务获得 `contents: write`，该任务不检出或执行项目代码。CI 不配置真实 `AIGATE_TOKEN`。

2026-10-12 首次远端 [CI](https://github.com/davisye/aigate-cli/actions/runs/38239804112) 已通过：`Linux checks` 完成全部自动测试、安装回归和 release 构建，`Dependency audit` 完成锁文件审计。

### GitHub 仓库设置

维护者在目标仓库检查以下设置；本地配置文件不能代替网页端设置：

1. 默认分支为 `main`。保护主分支：要求 PR 和成功的 `Linux checks`、`Dependency audit` 检查，禁止强推与删除。先运行 CI，再从实际检查记录中选择；单人维护可不要求其他人的审批。
2. Actions 默认工作流权限为读取；限制可使用的 Actions，并启用完整 SHA 固定策略。使用 GitHub 托管 runner。
3. 启用 Dependabot alerts 和安全更新。按仓库类型与账户方案核对 secret scanning、push protection 和 Private vulnerability reporting 的可用性；凭证扫描不能保证识别云扉自定义 Token。
4. 采用固定滚动 `nightly` 时，仓库必须关闭 immutable releases；此设置作用于整个仓库，新正式发行版也不会获得该项保护。若希望启用 immutable releases，应改用每次唯一的 nightly 标签。正式发行仍不覆盖既有版本，公开发布前上传齐全附件，发布后修复产物使用新版本。

设置依据：[分支保护](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)、[Actions 安全](https://docs.github.com/en/actions/reference/security/secure-use)、[仓库安全](https://docs.github.com/en/repositories/creating-and-managing-repositories/best-practices-for-repositories)、[不可变发行版](https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases)。

仓库可见性或账户方案变化后，重新核对这些设置。安全报告入口见 [安全政策](../SECURITY.md)，第三方 Actions 的允许列表及合并规则应与当前工作流一致。

### 准备源码发行

1. 核对平台状态、许可、README 与安全政策，填入真实仓库地址。检查 `cargo package --list --allow-dirty --locked`，确保包含必要文件，排除本地配置和构建产物。
2. 运行 `AIGATE_REQUIRE_SHELLS=1 make check`、`make build` 和 `cargo audit --file Cargo.lock`。源码安装的版本输出应为 `aigate-cli 0.1.0`。
3. 在决定准备发行时，将 `CHANGELOG.md` 的 `Unreleased` 内容归档到 `## [0.1.0] - YYYY-MM-DD`，保留新的 `Unreleased` 节。同步文档中未发布的状态表述；日期使用实际准备日期。提交全部发行内容并合入 `main`，再为该提交创建和推送 `v0.1.0` 标签。
4. 从 `main` 手动运行 **Prepare release**，输入 `v0.1.0`。流程验证标签与 Cargo 版本、主分支归属及已归档的发行说明，对标签对应提交重新执行 CI，然后生成源码包和 `SHA256SUMS`，最后创建 GitHub Release 草稿。已有同名 Release 会失败，不覆盖附件。
5. 核对草稿说明、对应提交和全部附件，下载并验证校验和后，再由维护者明确发布。工作流不会自动公开发行版。

本地可在干净且与标签一致的检出目录运行：

```sh
bash scripts/prepare-release.sh --check v0.1.0
bash scripts/prepare-release.sh v0.1.0
```

输出写入被忽略的 `dist/`，包含 `aigate-cli-0.1.0-source.tar.gz`、`SHA256SUMS`、`RELEASE_NOTES.md`。脚本不创建提交或标签，不覆盖已有产物。源码包来自 `git archive`，相同提交重复生成的压缩包一致；这不代表跨平台编译出的二进制逐字节一致。

### Nightly 自动发行

`.github/workflows/nightly.yml` 每天 UTC 19:23（北京时间次日 03:23）调度，也支持手动触发。仅在上游仓库 `main` 上运行，固定本次事件的完整提交 SHA，再对该提交运行 CI 和三平台原生 release 构建。定时运行遇到已发布的相同提交时跳过；手动运行始终重建。Nightly 使用仓库固定的 Rust 工具链，名称不表示使用 Rust nightly 编译器。

维护者首次启用时：

1. 确认接受滚动发行的覆盖语义，并在仓库 Settings 中关闭 immutable releases，再创建 Actions repository variable `NIGHTLY_ROLLING_ENABLED`，值为 `true`。变量表示维护者已核对设置；未启用变量时发布脚本直接失败，已存在的不可变 nightly 也会被拒绝。设置查询 API 需要管理员权限，内置 `GITHUB_TOKEN` 不能读取，因而工作流不查询或自行修改该设置，也不需要管理员 Token。不要在保持不可变设置开启时启用此变量，否则首次发布的 nightly 会被锁定。
2. 将工作流与脚本合入 `main`。Actions 使用现有 SHA 固定的官方 checkout / artifact Actions；只有发布 job 申请 `contents: write`，使用内置 `GITHUB_TOKEN`，无需添加 PAT 或 AIGate Token。
3. 在 **Actions → Nightly → Run workflow** 选择 `main` 手动运行，检查三平台测试、打包与离线运行结果。首次远端运行通过前，不将 macOS / Windows 状态标记为已验证。
4. 下载 nightly 附件并核对校验和、`--version` 和包内 `BUILD-INFO.txt`；桌面凭证库与真实 API 仍按平台验收步骤单独验证。

产物包括 Linux x86_64 GNU、Windows x64 MSVC、macOS Apple Silicon 二进制包、准确提交的源码包和 `SHA256SUMS`。附件名固定，下载入口见 [README](../README.md#nightly-预编译版本)。Nightly 的 `--version` 形如 `aigate-cli 0.1.0-nightly.YYYYMMDD+COMMIT`，由构建环境的 `AIGATE_BUILD_VERSION` 嵌入；普通 Cargo 安装仍显示包版本。

`scripts/publish-nightly.sh` 先校验本地产物，再上传到临时草稿并重新下载校验。全部完成后才删除旧 nightly Release（保留标签）、将 `nightly` 标签移动到本次提交，并把准备好的草稿发布为固定 `nightly` prerelease，显式设置 `latest=false`。普通 CI、构建或上传校验失败不会替换旧 nightly。

GitHub 不提供 Release 原子替换，最终切换阶段固定链接可能短暂不可用。若最终标签更新或发布失败，脚本保留已上传的草稿并输出其名称；维护者检查后重新运行 Nightly，成功后清理遗留的 `nightly-staging-*` 草稿。不要以同名稳定发行版或不可变发行版占用 `nightly`，脚本会拒绝替换。下载正好跨越更新时，校验不一致应重新下载包和校验和。
