use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use ed25519_dalek::VerifyingKey;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub database: PathBuf,
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub verifying_key_hex: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
}

impl ServiceConfig {
    pub fn parse(content: &str) -> Result<Self> {
        let config: Self = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("读取服务配置失败: {}", path.display()))?;
        Self::parse(&content).with_context(|| format!("解析服务配置失败: {}", path.display()))
    }

    pub fn server_mode(&self) -> bool {
        self.server_url.is_some()
    }

    pub fn legacy_verifying_key(&self) -> Result<Option<VerifyingKey>> {
        self.verifying_key_hex
            .as_deref()
            .map(parse_verifying_key)
            .transpose()
    }

    pub fn data_directory(&self) -> Result<&Path> {
        self.database
            .parent()
            .ok_or_else(|| anyhow::anyhow!("database 路径缺少父目录"))
    }

    pub fn installation_id_path(&self) -> Result<PathBuf> {
        Ok(self.data_directory()?.join("installation-id"))
    }

    pub fn device_token_path(&self) -> Result<PathBuf> {
        Ok(self.data_directory()?.join("secrets").join("device-token"))
    }

    fn validate(&self) -> Result<()> {
        if let Some(server_url) = &self.server_url {
            if server_url.trim().is_empty() {
                bail!("server_url 不能为空");
            }
        }
        if self.server_url.is_none() {
            if self
                .verifying_key_hex
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                bail!("离线模式必须配置 verifying_key_hex");
            }
            if self
                .device_id
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
            {
                bail!("离线模式必须配置 device_id");
            }
        }
        if let Some(key) = self.verifying_key_hex.as_deref() {
            parse_verifying_key(key)?;
        }
        Ok(())
    }
}

fn parse_verifying_key(value: &str) -> Result<VerifyingKey> {
    let bytes = hex::decode(value.trim()).context("管理员公钥不是十六进制")?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("管理员公钥必须是 32 字节"))?;
    VerifyingKey::from_bytes(&key).context("管理员公钥无效")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_does_not_require_a_key_or_legacy_device_id() {
        let config = ServiceConfig::parse(
            r#"
database = "C:\\ProgramData\\Anchor Guard\\audit.db"
server_url = "https://anchor.example.com"
"#,
        )
        .unwrap();
        assert!(config.server_mode());
        assert!(config.legacy_verifying_key().unwrap().is_none());
    }

    #[test]
    fn legacy_config_still_requires_key_and_device_id() {
        let missing = ServiceConfig::parse(
            r#"
database = "C:\\ProgramData\\Anchor Guard\\audit.db"
"#,
        );
        assert!(missing.is_err());

        let config = ServiceConfig::parse(
            r#"
database = "C:\\ProgramData\\Anchor Guard\\audit.db"
verifying_key_hex = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
device_id = "laptop-1"
"#,
        )
        .unwrap();
        assert!(!config.server_mode());
        assert!(config.legacy_verifying_key().unwrap().is_some());
        assert_eq!(
            config.installation_id_path().unwrap(),
            PathBuf::from(r"C:\ProgramData\Anchor Guard\installation-id")
        );
    }

    #[test]
    fn invalid_public_key_is_rejected() {
        let config = ServiceConfig::parse(
            r#"
database = "audit.db"
verifying_key_hex = "zz"
device_id = "laptop-1"
"#,
        );
        assert!(config.is_err());
    }
}
