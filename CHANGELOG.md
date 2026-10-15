# 更新记录

尚未发布首个正式版本；beta、RC 与 nightly 均为预发行版。

## [Unreleased]

## [0.1.1-beta.1] - 2026-10-15

### 功能

- `init <shell-type>` 输出末尾新增对应 shell 的配置文件位置与初始化示例注释，覆盖 Bash、Zsh、Fish、PowerShell、Clink/Cmder 与 Nushell。
- PowerShell 配置示例使用 `Invoke-Expression (&aigate-cli init powershell | Out-String)`，合并多行初始化脚本后执行。

### 发行范围

- 版本化发行新增 GPG 校验和签名与公开密钥附件，发布前重新下载并验证签名及文件哈希；沿用四平台原生 CI、打包与预发行流程。
- 生成 API 的真实账户任务与上传、Clink 的 Windows 交互式 Tab 仍待验收。

## [0.1.0-rc.1] - 2026-10-15

### 功能

- 新增 Workflow API：独立 Keyring 登录、文件上传、任务提交与状态及结果查询。
- 新增 Comfy API：应用和绑定实例查询、图片上传、任务提交与状态及结果查询；实例上传不携带平台 Token。
- 生成任务支持离线预览和明确确认，两套 API 的子命令及媒体类型加入 shell 补全；新增 API mock 与命令行测试。
- 新增 PowerShell 7+、Clink/Cmder 与 Nushell 离线补全，覆盖多级命令、别名、选项和固定值；README 补齐加载方法与生成 API 用法。

### 修复

- 默认脱敏覆盖 `comfyuiHost` 等主机字段中的 URL 查询凭证。
- 修复大写或混合大小写 HTTP(S) 链接漏脱敏；任务 ID 拒绝 URL 与占位值，标量创建响应仅接受正 int64 ID，参数错误不回显误传的签名链接。
- 修复 Clink 在补完选项值后无法返回父解析器、继续补全后续参数的问题。

### 发行范围

- 新增 `vX.Y.Z-rc.N` 标签发行，沿用 beta 的四平台原生 CI、二进制/源码包与 SHA256 校验流程，发布为非 Latest 的预发行版。
- Workflow 与 Comfy API 已通过本地 mock 测试，真实账户任务与上传仍待验收；Clink 的 Windows 交互式 Tab 仍待验收。

## [0.1.0-beta.1] - 2026-10-15

### 功能

- 提供区域、GPU 规格、个人与社区镜像、实例列表、详情和状态查询。
- 新增不带 ID 的 `status`：自动读取全部计费中实例，显示状态表、数量汇总及脱敏 JSON；保留 `status INSTANCE_ID` 单实例查询。
- 提供实例创建、开机、关机和释放；创建与释放支持离线预览及明确确认。
- 支持系统 Keyring 登录、临时环境凭证、默认脱敏的 JSON 与表格输出。
- 支持由用户配置加载的 Bash、Zsh、Fish 离线补全。

### 修复

- 将 rustls 锁定版本升级到 0.23.46，修复 RUSTSEC-2026-0346、0347、0348、0349 通告涉及的问题。
- 严格校验创建响应的正 int64 实例 ID，无效响应报告结果不明且不自动重试。
- 清理 API 错误信息中的终端控制字符，保留凭证脱敏。
- 状态总览拒绝无效或跨页重复的实例 ID，避免误报完整汇总。
- 清理单实例状态输出及开关机拒绝提示中的终端控制字符，开关机错误继续隐藏当前 Token。
- 统一镜像参数与无效 API origin 的错误退出码为 `2`，离线预览也拒绝包含凭证的 URL。

### 发行范围

- 新增 Linux aarch64 GNU 原生 CI、release 构建、打包及离线运行检查，nightly 与 beta 使用同一四平台构建配置。
- 推送与 Cargo 版本一致的 `vX.Y.Z-beta.N` 标签后，全部检查通过才自动发布 beta 预发行版；提供四个平台的二进制、源码包和 SHA256 校验和。
- 以 Cargo 作为 Windows、macOS、Linux 的主要源码构建与安装入口，保留 Linux Make 快捷入口。
- 新增三平台原生 CI 检查和定时 / 手动 nightly 工作流：固定 prerelease 下载地址、二进制及源码包、SHA256 校验和、含日期与提交号的版本标识。
- Beta 提供 Linux x86_64 / aarch64、Windows x64、macOS Apple Silicon 预编译包与源码；完整桌面凭证库实机验收范围仍为 Arch Linux x86_64。
- macOS、Windows 的原生 CI 已通过；桌面凭证库、真实 API 及其他系统版本仍待对应环境验收。Nightly 预编译包状态见 GitHub Releases。
