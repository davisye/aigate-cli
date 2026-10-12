# 安全政策

## 支持范围

首个计划发行版本为 v0.1.0，当前尚未发布。发布后优先维护最新发行版本；支持的平台与验证边界见 [平台说明](docs/platforms.md)。

## 报告安全问题

请通过本仓库 GitHub **Security → Report a vulnerability** 私密报告凭证泄漏、确认绕过、请求重放或其他安全问题。该入口需要仓库维护者启用 Private vulnerability reporting；若入口尚不可用，请先通过普通 Issue 请求启用私密报告渠道，只说明需要安全联系，不公开漏洞细节。仓库地址为 [davisye/aigate-cli](https://github.com/davisye/aigate-cli)，私有阶段仅授权成员可访问。

报告可包含受影响版本、操作系统、脱敏后的复现步骤、预期与实际行为。不要上传 Token、密码、Keyring 导出、完整实例连接信息或未脱敏的响应。普通 Issue 用于不含安全细节的问题。

如凭证已经泄露，请先在云扉撤销或更换 Token。`aigate-cli logout` 只删除本地凭证，不撤销服务端 Token。

## 项目安全约束

- Token 保存在系统凭证库或由运行环境临时提供；不写入源码、命令参数、明文配置或 CI。
- 创建和释放要求明确确认。写操作结果不明时先查状态，不自动重试。
- 官方 Keyring 凭证仅用于官方 API origin，禁止跟随重定向。
- 自动测试使用本地 mock。真实云资源测试须另外获得对应操作的授权。

系统凭证库不能保证同一用户进程间的应用隔离。更多说明见 [README](README.md#凭证与安全边界)。
