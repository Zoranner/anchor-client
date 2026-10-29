//! 安装标识和服务端设备令牌的本地持久化。

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallationIdentity {
    pub installation_id: Uuid,
}

impl InstallationIdentity {
    /// 读取安装标识；文件不存在时生成并原子写入。
    pub fn load_or_create(path: &Path) -> Result<Self> {
        match read_installation_id(path) {
            Ok(installation_id) => Ok(Self { installation_id }),
            Err(error) if is_not_found(&error) => {
                let installation_id = Uuid::new_v4();
                write_text_atomic(path, &format!("{installation_id}\n"))?;
                Ok(Self { installation_id })
            }
            Err(error) => Err(error),
        }
    }

    pub fn as_string(&self) -> String {
        self.installation_id.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct DeviceTokenStore {
    path: PathBuf,
}

impl DeviceTokenStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> Result<Option<String>> {
        let content = match fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("读取设备令牌失败: {}", self.path.display()));
            }
        };
        let token = content.trim();
        validate_device_token(token)?;
        Ok(Some(token.into()))
    }

    pub fn save(&self, token: &str) -> Result<()> {
        validate_device_token(token)?;
        anchor_system::secure_file::write_private_file(&self.path, format!("{token}\n").as_bytes())
            .map_err(anyhow::Error::new)
            .with_context(|| format!("保存设备令牌失败: {}", self.path.display()))
    }

    pub fn clear(&self) -> Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("删除设备令牌失败: {}", self.path.display()))
            }
        }
    }
}

pub fn validate_device_token(token: &str) -> Result<()> {
    if token.len() != 64 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("设备令牌必须是 64 位十六进制字符串");
    }
    Ok(())
}

fn read_installation_id(path: &Path) -> Result<Uuid> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("读取安装标识失败: {}", path.display()))?;
    let value = content.trim();
    Uuid::parse_str(value).with_context(|| format!("安装标识不是 UUID: {}", path.display()))
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
}

fn write_text_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("安装标识路径缺少父目录"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("创建数据目录失败: {}", parent.display()))?;
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("安装标识文件名无效"))?;
    let temporary = path.with_file_name(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("创建安装标识临时文件失败: {}", temporary.display()))?;
        file.write_all(contents.as_bytes())
            .context("写入安装标识失败")?;
        file.sync_all().context("同步安装标识失败")?;
        fs::rename(&temporary, path).context("发布安装标识失败")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_identity_is_created_once_and_reused() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("identity").join("installation-id");

        let first = InstallationIdentity::load_or_create(&path).unwrap();
        let second = InstallationIdentity::load_or_create(&path).unwrap();

        assert_eq!(first, second);
        assert!(!first.as_string().is_empty());
    }

    #[test]
    fn invalid_installation_id_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("installation-id");
        fs::write(&path, "not-a-uuid").unwrap();

        assert!(InstallationIdentity::load_or_create(&path).is_err());
    }

    #[test]
    fn token_validation_requires_a_sha256_sized_hex_value() {
        assert!(validate_device_token(&"a".repeat(64)).is_ok());
        assert!(validate_device_token(&"a".repeat(63)).is_err());
        assert!(validate_device_token(&"z".repeat(64)).is_err());
    }

    #[test]
    fn missing_token_and_clear_are_not_errors() {
        let directory = tempfile::tempdir().unwrap();
        let store = DeviceTokenStore::new(directory.path().join("missing-token"));

        assert_eq!(store.load().unwrap(), None);
        assert!(store.clear().is_ok());
    }
}
