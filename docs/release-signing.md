# 版本化发行的 GPG 签名

本流程为 `vX.Y.Z-alpha.N`、`vX.Y.Z-beta.N`、`vX.Y.Z-rc.N` 和 `vX.Y.Z` 生成 `SHA256SUMS.asc`。签名覆盖校验和清单，清单再覆盖源码包及该次发行提供的二进制包。Nightly 继续使用原有滚动发行流程。

alpha / beta / rc 通过检查、打包、签名和重新下载校验后自动公开为 prerelease；正式稳定版仍只生成源码发行草稿，由维护者检查后发布。每个版本标签和附件只发布一次，不覆盖旧版。

签名主钥完整指纹：

```text
4DB25C5D9BB79A946A5DBAD9125042501C150D43
```

CI 使用该身份下单独的发布签名子钥，不能使用日常个人签名子钥，也不能导入主钥私钥。GPG 文件签名与 Windows Authenticode、macOS 代码签名及公证是不同的验证机制。

当前 CI 签名子钥完整指纹为 `F9997B4F157224D7AB95342B72191B6B0C3B667C`，有效期至 2027-10-15。`release-signing` Environment 已配置两个签名 Secrets、完整子钥指纹变量，以及 `main` 分支 / `v*` 标签访问规则。口令由本机随机生成，另存本机 GNOME Keyring；主钥私钥及个人 GPG 口令未上传 CI。真实发行工作流仍需在包含本改动的版本标签上验收。

## 首次启用

工作流和签名脚本合入主分支并不表示已启用：还需要先生成 CI 专用签名子钥、保存更新后的完整备份，再配置 GitHub Environment。缺少密钥、口令或签名指纹时，发布 job 会失败，不能退回无签名发布。

### 准备专用签名子钥

当前 CI 子钥已配置，以下步骤供首次配置或重建时使用。本机主钥维护目录已在更新 Bitwarden 备份后清理；需要维护时，先从 Bitwarden 完整备份的私钥区块恢复到权限为 0700 的独立临时维护密钥库，不导入吊销证书。下面的维护路径仅作为恢复后的示例。

在恢复后的个人主钥维护密钥库中添加一个有效期为一年的 Ed25519 签名子钥：

```bash
gpg --homedir /home/davisye/.local/share/gpg-signing/ye-bingli/master --quick-add-key 4DB25C5D9BB79A946A5DBAD9125042501C150D43 ed25519 sign 1y
gpg --homedir /home/davisye/.local/share/gpg-signing/ye-bingli/master --with-subkey-fingerprint --list-secret-keys 4DB25C5D9BB79A946A5DBAD9125042501C150D43
```

记录新子钥的完整指纹。使用 `--export-secret-subkeys '新子钥完整指纹!'` 导出时，必须保留结尾的 `!`，只选择这个 CI 子钥。不要使用 `--export-secret-keys` 向 CI 导出完整主钥；不要把私钥放进项目或同步源码目录。

建议在权限为 0700 的另一个独立临时密钥库中导入这一份子钥导出，确认显示 `sec#` 和唯一的 CI `ssb`，再用 `gpg --homedir 临时密钥库 --change-passphrase 'CI子钥完整指纹!'` 为这一份副本设置独立 CI 口令。这样日常个人口令无需进入 CI。

在仅保留子钥私钥的密钥库中，更换口令可能因主钥私钥缺失而打印 `No secret key` 并以状态 2 退出，同时返回 `SUCCESS keyedit.passwd`。出现这种情况时，先停止上传；退出码和这条状态都不足以证明新副本可用。重新导出 CI 子钥副本，在一个新的临时密钥库中导入，用新 CI 口令签署一次性文件，并确认签名中的完整子钥指纹与预期一致。正常退出时也须完成这项验证。验证通过后才替换 GitHub Secrets，并清理临时密钥库和导出文件。

添加子钥后重新导出完整主钥备份并更新 Bitwarden 安全笔记，保留原有吊销证书。更新公开公钥，保留历史签名子钥的公钥信息。完成新备份的恢复验证后，才清理本机主钥维护副本及冗余私钥导出。

### GitHub 配置

在仓库 **Settings → Environments** 创建 `release-signing`。Deployment branches and tags 使用 selected branches and tags：允许 `main` 分支供手动运行，允许 `v*` 标签供自动发行。标签格式、Cargo 版本、主分支归属和发行说明仍由 validate job 严格检查。

仅在这个 Environment 中添加：

| 类型 | 名称 | 内容 |
| --- | --- | --- |
| Secret | `RELEASE_GPG_PRIVATE_KEY` | 仅 CI 签名子钥的 ASCII armored 私钥导出，含主钥公钥及私钥占位信息 |
| Secret | `RELEASE_GPG_PASSPHRASE` | CI 导出副本的独立口令 |
| Variable | `RELEASE_GPG_SIGNING_FINGERPRINT` | CI 签名子钥的完整指纹，不是主钥或日常子钥指纹 |

也可用 CLI 上传私钥文件，并交互输入口令；口令不要放在命令参数里：

```bash
gh secret set RELEASE_GPG_PRIVATE_KEY --repo davisye/aigate-cli --env release-signing < /安全目录/ci-signing-subkey.asc
gh secret set RELEASE_GPG_PASSPHRASE --repo davisye/aigate-cli --env release-signing
gh variable set RELEASE_GPG_SIGNING_FINGERPRINT --repo davisye/aigate-cli --env release-signing --body 'CI子钥完整指纹'
```

只签名的发布 job 访问上述密钥。PR、构建和测试 job 不获得签名 Secrets。无需添加 PAT 或真实 AIGate 凭据；发布仍使用内置 `GITHUB_TOKEN`。Environment 的规则和密钥必须在 GitHub 设置中配置，代码无法替代这些设置。

依据：[GitHub Environment secrets](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments)、[Actions Secrets](https://docs.github.com/en/actions/security-for-github-actions/security-guides/using-secrets-in-github-actions)、[GnuPG 密钥导出](https://www.gnupg.org/documentation/manuals/gnupg/GPG-Input-and-Output.html)。

## 发布流程

1. 将 Cargo 包及锁文件版本更新为待发布版本，归档同名 CHANGELOG 版本说明，完成 CI 并合入 `main`。
2. 创建与 Cargo 版本完全一致的签名标签，例如 `v0.2.0-alpha.1`、`v0.2.0-beta.1`、`v0.2.0-rc.1` 或 `v0.2.0`，推送标签。也可从 `main` 手动运行 Release 并输入已有标签。
3. 检查 Release 工作流。预发行版构建四目标二进制和源码包；正式版保持源码草稿流程。最终校验和生成后，使用临时密钥库导入 CI 子钥，签署并验证 `SHA256SUMS.asc`，导出公开的 `aigate-cli-release-key.asc`。临时密钥库在成功或失败时都清理。
4. 预发行版先创建草稿，重新下载附件，对比签名、公开密钥和清单，并检查所有包的哈希；全部通过后公开。正式版由维护者下载验签并明确发布。

正常产物包含：发行包、`SHA256SUMS`、`SHA256SUMS.asc`、`aigate-cli-release-key.asc`。以前发布的 `v0.1.0-beta.1` 和 nightly 不会因新流程自动补签。

## 下载者验证

从同次发行下载所需包及上述三个验证文件。先将公钥完整指纹与本页通过可信渠道取得的指纹对照，再导入公钥，验证签名，最后检查文件哈希：

```bash
gpg --show-keys --with-fingerprint aigate-cli-release-key.asc
gpg --import aigate-cli-release-key.asc
gpg --verify SHA256SUMS.asc SHA256SUMS
sha256sum --check --ignore-missing SHA256SUMS
```

签名成功表示清单由对应密钥签署；用户仍需确认主钥指纹。`--ignore-missing` 适合只下载当前平台的包，至少应有一个待校验文件；不要忽略不匹配的哈希。

## 维护和验收

- 子钥到期前更新有效期，重新导出 CI 子钥并替换 Environment Secret；新导出的口令要与 `RELEASE_GPG_PASSPHRASE` 匹配。轮换子钥时，同时更新完整签名指纹、备份和公钥发布位置。
- 泄露时撤销这个 CI 子钥，移除旧 Secrets、生成替代子钥并公告受影响范围；个人日常子钥保持独立。只更新 GitHub 密钥状态不能撤回已下载的旧公钥，撤销信息需要公开发布。
- 本地运行 `bash tests/release.sh`、`bash tests/release-signing.sh` 和 ShellCheck。签名回归使用一次性密钥，验证正常签名、篡改拒绝、缺失 Secrets、错误指纹、错误口令、主钥私钥误导入及临时密钥清理。
- 首次真实验收需要使用一个新的正常版本标签；本地通过不能代替远端四平台构建、Secrets 接入和下载验签。不要为验证 CI 覆盖已有公开版本。
