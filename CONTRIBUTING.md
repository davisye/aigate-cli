# 参与贡献

AIGate CLI 面向个人账户的资源与实例管理。提交问题前请查看 [README](README.md)、命令帮助和 [已知限制](docs/development.md#已知行为与验证边界)。Workflow API 与 Comfy API 属于后续计划。

## 问题反馈

请提供 CLI 版本、系统与 shell 版本、脱敏命令、退出码及预期行为。不要提供 Token、密码或完整连接信息；安全问题按 [安全政策](SECURITY.md) 私密报告。

## 修改与验证

1. 从 `main` 创建分支，说明改动目的和用户可见影响。
2. 保留创建与释放确认、官方凭证绑定、默认脱敏和禁止自动重试的约束。
3. API 行为变化需补充本地 mock 测试；不得把真实账户或云资源操作加入 CI。
4. Linux 安装构建依赖、Git、jq、Bash、Zsh、Fish 后运行 `AIGATE_REQUIRE_SHELLS=1 make check` 和 `make build`。完整步骤见 [开发文档](docs/development.md#自动测试)。
5. 用户可见变化记入 [CHANGELOG](CHANGELOG.md) 的 `Unreleased` 部分，必要时同步 README 与平台说明。

工具链由 `rust-toolchain.toml` 固定，用于一致的开发与发行检查，不代表已验证最低 Rust 版本。新增平台必须区分编译成功、原生凭证验证和实际运行验收。

提交贡献表示同意贡献按本项目的 [Apache License 2.0](LICENSE) 分发。引入第三方代码或资源时保留适用的许可、版权及声明。
