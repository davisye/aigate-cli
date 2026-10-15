# 第三方来源与许可

本项目自身采用 [Apache License 2.0](LICENSE)。该许可不改变第三方代码、数据或依赖的许可。本文件记录参考来源与依赖核对范围，不替代依赖随附的完整许可文本。

## 参考项目

### AutoDL CLI

- 来源：用户提供的 AutoDL CLI 参考项目。
- 用途：CLI 命令组织、环境变量优先于 Keyring、JSON/表格输出和测试组织的参考。
- 核对结果：参考项目的 `Cargo.toml` 标注 `Apache-2.0`，附 Apache-2.0 `LICENSE`；未发现 `NOTICE` 文件。
- 当前项目的认证加入了 Token 严格校验与官方 origin 绑定，输出使用云扉字段与递归脱敏，未随项目纳入 AutoDL CLI 的完整源文件或资源文件。保留以上来源归属；未来引入或修改其代码时，应一并保留适用的版权、许可和声明，并注明修改。

### OPC

- 来源：[LLsetnow/OPC，提交 `7e2aef98138940a29bea7e3aee38dae13e8dcca7`](https://github.com/LLsetnow/OPC/tree/7e2aef98138940a29bea7e3aee38dae13e8dcca7)。
- 用途：核对区域、分页、个人/社区镜像及实例管理的接口行为；请求格式与状态约束另外依据 [云扉官方 OpenAPI](https://docs.aigate.cc/developer/openAPI) 实现。
- 核对结果：该提交的文件树没有许可文件或 `NOTICE`，`pyproject.toml` 也未声明许可证。当前项目不包含其 Python 源码、测试文件或资源；公开可见不代表可按 Apache-2.0 再分发。后续如需引入其实现，须先取得相应许可。

## Rust 依赖

版本来源为 [Cargo.lock](Cargo.lock)，声明来源为各 crate 的 Cargo 元数据及随附许可。2026-10-12 对 Linux x86_64 目标的正常依赖和构建依赖进行了元数据核对；全部可达条目均有许可表达式。该清单包含构建工具、宏和可能受特性统一影响的条目，不等同于最终二进制链接清单，也不覆盖 Windows/macOS 的全部目标依赖。

除常见的 MIT / Apache-2.0 双许可外，需保留和核对以下声明：

| 依赖 | 当前锁定版本的许可表达式 |
| --- | --- |
| `ring` | `Apache-2.0 AND ISC`，应核对 crate 内的完整许可与来源声明 |
| `rustls-webpki`、`untrusted` | ISC |
| `subtle` | BSD-3-Clause |
| `webpki-roots` | CDLA-Permissive-2.0 |
| ICU 数据及相关 Unicode crates | Unicode-3.0 |
| `unicode-ident` | `(MIT OR Apache-2.0) AND Unicode-3.0` |

源码仓库没有 vendored 依赖源码。分发编译后的程序时，应针对对应目标和实际启用特性整理第三方许可文本、版权及适用的 `NOTICE`，随产物提供；依赖有多重或附加许可时，不能用本项目的 Apache-2.0 文件替代。升级依赖或增加平台后需重新核对。Beta 与 nightly 已提供二进制发行包，RC 沿用同一打包流程；上述元数据核对范围仍不代表全部目标的完整链接清单。
