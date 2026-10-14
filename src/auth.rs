use crate::client::DEFAULT_BASE;
use anyhow::{bail, Context, Result};
use reqwest::Url;
use std::io::{self, IsTerminal, Read};

const SERVICE: &str = "aigate-cli";
const ACCOUNT: &str = "default";
pub const WORKFLOW_BASE: &str = "https://studio.aigate.cc";

#[derive(Clone, Copy)]
pub enum Profile {
    Compute,
    Workflow,
}

impl Profile {
    pub fn origin(self) -> &'static str {
        match self {
            Self::Compute => DEFAULT_BASE,
            Self::Workflow => WORKFLOW_BASE,
        }
    }

    pub fn environment(self) -> &'static str {
        match self {
            Self::Compute => "AIGATE_TOKEN",
            Self::Workflow => "AIGATE_WORKFLOW_TOKEN",
        }
    }

    fn account(self) -> &'static str {
        match self {
            Self::Compute => ACCOUNT,
            Self::Workflow => "workflow",
        }
    }

    fn login_command(self) -> &'static str {
        match self {
            Self::Compute => "aigate-cli login",
            Self::Workflow => "aigate-cli workflow login",
        }
    }

    pub fn require_keyring_origin(self, base: &Url) -> Result<()> {
        if base.origin().ascii_serialization() != self.origin() {
            bail!("Keyring 凭证仅适用于官方 API origin；自定义 origin 需用 {} 临时提供对应凭证，login/logout 仅管理官方凭证", self.environment());
        }
        Ok(())
    }
}

// Keep the existing entry for compatibility; it belongs only to the official origin.
pub fn require_keyring_origin(base: &Url) -> Result<()> {
    Profile::Compute.require_keyring_origin(base)
}

// Deliberately no Debug implementation: credentials must never enter diagnostics.
pub struct Token(String);

impl Token {
    pub fn parse(raw: &str) -> Result<Self> {
        let trimmed = raw.trim();
        if trimmed.eq_ignore_ascii_case("bearer") {
            bail!("Bearer 前缀后缺少 Token");
        }
        let value = if trimmed
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("bearer "))
        {
            trimmed[7..].trim()
        } else {
            trimmed
        };
        if value.is_empty()
            || !value.is_ascii()
            || value
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            bail!("Token 必须是非空、单行且不含空白的 ASCII 字符串");
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

fn entry(profile: Profile) -> Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, profile.account()).context("无法访问系统 Keyring")
}

pub fn resolve(base: &Url) -> Result<Token> {
    resolve_for(Profile::Compute, base)
}

pub fn resolve_for(profile: Profile, base: &Url) -> Result<Token> {
    let environment = profile.environment();
    match std::env::var(environment) {
        Ok(raw) => Token::parse(&raw),
        Err(std::env::VarError::NotUnicode(_)) => bail!("{environment} 编码无效"),
        Err(std::env::VarError::NotPresent) => {
            profile.require_keyring_origin(base)?;
            match entry(profile)?.get_password() {
                Ok(raw) => Token::parse(&raw),
                Err(keyring::Error::NoEntry) => {
                    bail!(
                        "未找到 Token，请运行 `{}` 或设置 {environment}",
                        profile.login_command()
                    )
                }
                Err(_) => bail!("无法读取系统 Keyring；请解锁 Keyring 或设置 {environment}"),
            }
        }
    }
}

pub fn login() -> Result<()> {
    login_for(Profile::Compute)
}

pub fn login_for(profile: Profile) -> Result<()> {
    let raw = if io::stdin().is_terminal() {
        rpassword::prompt_password("AIGate Access Token（输入隐藏）: ").context("无法读取 Token")?
    } else {
        let mut raw = String::new();
        io::stdin()
            .take(16 * 1024)
            .read_to_string(&mut raw)
            .context("无法从标准输入读取 Token")?;
        raw
    };
    let token = Token::parse(&raw)?;
    entry(profile)?
        .set_password(token.expose())
        .map_err(|_| anyhow::anyhow!("无法写入系统 Keyring"))?;
    println!(
        "Token 已保存到系统 Keyring（{SERVICE}/{}）",
        profile.account()
    );
    warn_environment(profile);
    Ok(())
}

pub fn logout() -> Result<()> {
    logout_for(Profile::Compute)
}

pub fn logout_for(profile: Profile) -> Result<()> {
    match entry(profile)?.delete_credential() {
        Ok(()) => println!("已移除 Keyring 中的 Token"),
        Err(keyring::Error::NoEntry) => println!("Keyring 中没有已保存的 Token"),
        Err(_) => bail!("无法移除 Keyring 中的 Token"),
    }
    warn_environment(profile);
    Ok(())
}

fn warn_environment(profile: Profile) {
    let environment = profile.environment();
    if std::env::var_os(environment).is_some() {
        eprintln!(
            "{environment} 仍已设置，它优先于 Keyring；请在当前 shell 中清除该变量以使用 Keyring"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_credentials_are_bound_to_the_normalized_official_origin() {
        for raw in [DEFAULT_BASE, "https://WAAS.AIGATE.CC:443/"] {
            let base = crate::client::normalize_origin(raw).unwrap();
            assert!(require_keyring_origin(&base).is_ok());
        }
        for raw in [
            "https://example.invalid",
            "https://waas.aigate.cc:444",
            "https://other.waas.aigate.cc",
            "https://waas.aigate.cc.example.invalid",
            "http://127.0.0.1:8080",
        ] {
            let base = crate::client::normalize_origin(raw).unwrap();
            assert!(require_keyring_origin(&base).is_err());
        }
    }

    #[test]
    fn normalizes_bearer_without_echoing_invalid_credentials() {
        assert_eq!(
            Token::parse("  BeArEr example-token\n").unwrap().expose(),
            "example-token"
        );
        for raw in ["", "Bearer ", "a\nb", "a\rb", "a b", "abc\0xyz", "中文"] {
            assert!(Token::parse(raw).is_err());
        }
        let error = Token::parse("private\nsecret").err().unwrap().to_string();
        assert!(!error.contains("private"));
        assert!(!error.contains("secret"));
    }
}
