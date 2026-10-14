---
name: aigate-cli
description: 使用 aigate-cli 查询云扉 GPU、镜像和实例，执行已获授权的实例管理、Workflow 与 Comfy 任务提交、状态查询和文件上传，排查认证或命令失败。用户要求使用该 CLI 管理 AIGate 资源或生成任务时使用。
---

# AIGate CLI

依据当前用户请求确定账户和资源范围，复用已有认证。读取本技能不授权修改凭证或云资源；用户已明确授权本次具体操作时，落实既有授权，不重复询问。

## 定位与输出

先查看 `aigate-cli --version`、`aigate-cli --help`；具体参数以 `aigate-cli COMMAND --help` 为准。Bash/Zsh/Fish 用 `command -v aigate-cli` 定位，PowerShell 用 `Get-Command aigate-cli`。未安装或版本过旧时，在仓库根目录可用 `cargo run --locked -- <参数>`；安装说明按需读取 [README](../../README.md#构建与运行)。运行资源命令使用登录用户身份。

优先使用 `--json`，保留完整 ID。以下 `AREA`、`GPU_SKU`、`IMAGE_NAME`、`IMAGE_ID`、`INSTANCE_ID` 为占位符，替换为当前查询取得或用户指定的值。

## 认证

- 来源为 `AIGATE_TOKEN` → 系统 Keyring 的 `aigate-cli/default`。已设置但为空或无效的环境变量报错，不回退到 Keyring。
- Workflow 独立使用 `AIGATE_WORKFLOW_TOKEN` → `aigate-cli/workflow`，官方 origin 为 `https://studio.aigate.cc`；登录 / 退出命令为 `workflow login` / `workflow logout`。算力与 Comfy 共用原有登录，凭证不跨服务回退。以下算力凭证排查方式对 Workflow 使用对应变量和命令。
- Keyring 项仅用于归一化后的官方 origin `https://waas.aigate.cc`。自定义 `--base-url` 需显式临时提供该地址的环境凭证；`login/logout` 只管理官方凭证，不把正式 Token 搬到其他服务绕过限制。
- 用户指定 Keyring 时，Bash/Zsh/Fish 用 `env -u AIGATE_TOKEN aigate-cli ...` 排除覆盖。PowerShell 可先运行 `Remove-Item Env:AIGATE_TOKEN -ErrorAction SilentlyContinue`；均不读取或打印原值。
- 认证缺失或凭证库锁定时，让用户在本地终端解锁并用 `aigate-cli login` 隐藏输入。Token 不放入对话、代码、明文配置、命令参数或日志；没有 `--token` 选项，也无明文存储回退。
- `logout` 仅删除本地项，不撤销服务端 Token；不用于连通性测试。默认详情已脱敏，仅在用户需要连接凭证时使用 `--show-secrets`，避免带入报告或日志。
- Keyring 及调用侧确认不提供同一用户进程隔离或服务端权限控制，不能将长期全权限 Token 宣称为只读或短期凭证。

## 查询

| 目的 | 命令 |
| --- | --- |
| 离线区域列表 | `aigate-cli regions --json` |
| 两个区域的规格 | `aigate-cli skus --json` |
| 指定区域规格 | `aigate-cli skus --area AREA --json` |
| 全部个人镜像 | `aigate-cli images --all --json` |
| 社区镜像 | `aigate-cli images --image-type 2 --area AREA --sku GPU_SKU --name IMAGE_NAME --all --json` |
| 全部计费中实例 | `aigate-cli list --all --json` |
| 计费中实例状态总览 | `aigate-cli status --json` |
| 运行中实例 | `aigate-cli list --operation-status 2 --all --json` |
| 已释放实例 | `aigate-cli list --operation-status 4 --all --json` |
| 详情 / 状态 | `aigate-cli info INSTANCE_ID --json` / `aigate-cli status INSTANCE_ID --json` |

- `gpus`、`stock` 是 `skus` 的别名；规格可查询不代表此刻有可分配库存。价格和容量按 API 原值报告，不推测单位或扣费。
- `list`、`images` 默认只查一页（20 条）。`--all` 从第一页读到 `total`，与显式 `--page` 互斥；实例页大小为 1–20，镜像为 5–20。默认实例列表只含计费中实例，空列表可为有效结果。
- `status` 无 ID 时自动查询全部计费中实例并按状态汇总，无需 `--all`。汇总使用列表的 `operationStatus`；核验某个实例的生命周期终态时使用 `status INSTANCE_ID`。已释放历史记录通过列表筛选 `4` 查询。
- 实例 ID 字段为 `instanceId`，镜像 ID 为 `worksId`，均按正 int64 保留，不经过浮点数转换或复用历史测试 ID。
- 支持个人镜像 `3`、社区镜像 `2`，不接受系统镜像 `1`。官方环境需由用户先保存为个人镜像，再查询 ID。社区查询的区域、SKU、非空名称必填，版本可用 `--image-version` 过滤；个人查询不接受这些社区专用过滤项。

## 实例操作

| 用户意图 | 命令与约束 |
| --- | --- |
| 新建计费实例 | `create`；明确区域、镜像 ID、类型；`--sku` / `--gpu-model` 二选一；数量 1–8，默认 1 |
| 恢复 / resume / 重新开机 | `start INSTANCE_ID`；已有实例状态须为 `22` 或 `26`，没有 `resume` 子命令 |
| 关机并保留实例 | `stop INSTANCE_ID`；当前状态须为运行中 `2` |
| 删除实例资源 | `release INSTANCE_ID`，别名 `rm`；状态 `4` 为已释放，不能再开机 |

暂停或稍后恢复使用 `stop`。个人镜像与实例是独立资源，从镜像新建仍属于创建操作。

### 创建与释放

先核对具体参数或实例，输出离线预览：

```text
aigate-cli create --sku GPU_SKU --area AREA --image-id IMAGE_ID --image-type 3 --count 1 --dry-run --json
aigate-cli release INSTANCE_ID --dry-run --json
```

预览不读取凭证或请求 API。已有授权明确包含本次完整参数及创建，或该 ID 的释放与数据删除时，移除 `--dry-run`，添加 `--yes --json` 提交一次；授权不完整时先准备查询和预览，再请用户决定缺失项。查询、冒烟或关机不能解释为释放授权，不自行更换规格、增加数量或操作范围外实例。

`--yes` 与 `--dry-run` 互斥。非交互执行未传 `--yes` 会拒绝；交互创建输入 `create`，释放输入完整 ID。`--yes` 用于落实用户已给出的明确确认。

### 状态核验

创建后记录新 `instanceId`；开关机和释放只针对已核对 ID。提交后间隔查询 `status INSTANCE_ID`，设置有限等待时长，不重复提交写操作。运行中为 `2`，已释放为 `4`，初始化为 `9`，关机中为 `11`，开机中为 `13`，已关机为 `22`，关机且已上传为 `26`；其他码保留原值与 `label`，不能当作可开关机状态。

报告实际 ID、请求是否接受、最后观测状态和未核实项。`accepted: true` 不表示生命周期已完成；实际扣费须核对账单。

## Workflow 与 Comfy 生成任务

先查看 `aigate-cli workflow --help` 或 `aigate-cli comfy --help`。第一版支持以下命令；`APP_ID`、`INSTANCE_ID`、`TASK_ID` 与文件路径须来自当前用户指定或实际查询，不能复用历史示例：

| 目的 | 命令 |
| --- | --- |
| Comfy 应用及参数定义 | `aigate-cli comfy apps --json` |
| 应用绑定实例 | `aigate-cli comfy instances APP_ID --json` |
| Workflow 文件上传 | `aigate-cli workflow upload FILE --media-type image\|video\|audio` |
| Comfy 图片上传 | `aigate-cli comfy upload FILE --app-id APP_ID --instance-id INSTANCE_ID` |
| Workflow 提交预览 | `aigate-cli workflow run --app-id APP_ID --input params.json --dry-run` |
| Comfy 提交预览 | `aigate-cli comfy run --app-id APP_ID --instance-id INSTANCE_ID --input inputs.json --dry-run` |
| 任务状态与文件结果 | `aigate-cli workflow status TASK_ID` / `aigate-cli comfy status TASK_ID` |

Workflow 的 appId 和参数定义从 Studio 对应工作流页面取得，尚无 CLI 列表接口。`params.json` 只含 `inputParams` object；Comfy 的 `inputs.json` 只含应用 `allowInput` object，均不是完整工作流图或带外层 appId 的请求体。按定义保留参数类型，文件限 16 MiB。不要在共享文件中保留 Token 或连接凭证。

提交会消耗积分或实例资源，Workflow 排队任务不能取消。先核对用户授权的应用、实例、输入参数与费用范围，准备 `--dry-run`；明确授权后移除预览、添加 `--yes` 提交一次。授权不完整时只做查询和离线预览，再请求缺失决定。CLI 交互输入 `submit`；非交互未传 `--yes` 不发送请求。不得以提交任务为由自动创建、开机、关机或释放实例。

文件上传本身会向云端发送本地内容，须在用户指定文件和目标的范围内执行，支持 `--dry-run` 离线预览元数据。Workflow 图片限 5 MB、音视频限 20 MB。Comfy 上传先查询并唯一匹配应用、运行中的实例，使用独立无平台认证的客户端发送图片到 `input`；将返回的 `name` 按应用定义填入参数，不自行拼接或猜测实例 host。

提交返回 `taskId`（输出为字符串）；保存后间隔查询同一套 API，不重复提交。任务 ID 不能是 URL 或占位值；数字须为正 int64，其他标识仅含 ASCII 字母、数字、下划线和连字符，须以字母或数字开头。状态查询总是 JSON，保留字段并增加 `label`；退出码 0 只表示查询正常，任务成功须 `status=2`。`0` 排队、`1` 生成、Workflow 的 `6` 上传属于进行中；`2..5` 为终态，未知状态保留原值并待核对。`files` 可为 null 或空数组；Comfy 下载链接与实例绑定，应在关机或释放前保存结果。第一版没有等待、下载或取消命令。

签名 URL 和 `comfyuiHost` 查询串默认脱敏。只有用户确需取出结果链接时，在私密输出中对任务状态或 Workflow 上传使用 `--show-secrets`；不将结果写入报告或 CI 日志。文档未明确的创建、Workflow 查询和上传结构仍待真实 API 验收，边界见 [生成 API 验证边界](../../docs/development.md#生成-api-验证边界)。超时或响应异常先核对任务状态或控制台调用记录；没有任务 ID 时报告结果不明，不能自动重放。

## 结果与失败

- 分页 JSON 为 `{ "total", "records" }`；状态总览为 `{ "total", "summary", "records" }`，`summary` 每项含 `status`、`label`、`count`，记录增加 `label` 并脱敏。SKU 为数组，详情 / 创建为 API 的 `data`；单实例状态为 `{ "instanceId", "status", "label" }`，开关机 / 释放为 `{ "action", "instanceId", "accepted" }`，不含 API 外层 envelope。
- `--json` 可在子命令前后使用；提示与错误写入 stderr。`login/logout` 输出文字，`init` 始终输出 shell 代码。
- 退出码 `0` 为成功或预览，`1` 为凭证、确认、网络或 API 失败，`2` 为参数错误。保留脱敏错误和退出码，不能把失败当作空列表。
- CLI 不自动重试或跟随重定向。写请求超时、响应异常或资源不足时先查状态、列表或控制台，结果不明就报告待核实，不循环创建、开机或释放。
- `--base-url` 只接受 HTTPS origin，HTTP loopback 用于本地 mock。网络选项可用 `--no-proxy`、`--timeout 1..300`；修改网络参数不能作为重放结果不明操作的理由。

## 补全与按需参考

`aigate-cli init bash|zsh|fish` 离线输出脚本，不读取 Token 或请求 API。补全不在线查询实例 ID、镜像 ID 或 SKU；安装程序不自动安装补全或修改 shell 配置，尚无 PowerShell 补全。

- [README](../../README.md)：安装、登录及用户要求的 shell 配置。
- [开发与测试](../../docs/development.md)：排查实现或 API 映射、用户要求手动验收时读取；生命周期测试仍遵循当前授权。
- [平台支持](../../docs/platforms.md)：检查目标平台状态、原生凭证或终端差异时读取。
