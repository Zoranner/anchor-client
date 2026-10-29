//! 仅服务可读的本地文件写入。

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use crate::error::PlatformError;

#[cfg(windows)]
const PRIVATE_FILE_SDDL: &str = "D:P(A;;FA;;;SY)(A;;FA;;;BA)";
#[cfg(windows)]
use windows::Win32::Security::PSECURITY_DESCRIPTOR;

/// 原子写入一个仅 SYSTEM/Administrators 可访问的文件。
///
/// 父目录也会收紧 ACL，因此目录内的其他文件同样不可被普通用户读取。
pub fn write_private_file(path: &Path, contents: &[u8]) -> Result<(), PlatformError> {
    let parent = path
        .parent()
        .ok_or_else(|| PlatformError::Failed("私密文件路径缺少父目录".into()))?;
    fs::create_dir_all(parent).map_err(io_error)?;
    set_private_permissions(parent, true)?;

    let temporary = temporary_path(path);
    let result = (|| -> Result<(), PlatformError> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(io_error)?;
        file.write_all(contents).map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        set_private_permissions(&temporary, false)?;
        replace_file(&temporary, path)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("private-file");
    path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ))
}

fn io_error(error: std::io::Error) -> PlatformError {
    PlatformError::Failed(format!("写入私密文件失败: {error}"))
}

#[cfg(unix)]
fn set_private_permissions(path: &Path, directory: bool) -> Result<(), PlatformError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = if directory { 0o700 } else { 0o600 };
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).map_err(io_error)
}

#[cfg(windows)]
fn set_private_permissions(path: &Path, _directory: bool) -> Result<(), PlatformError> {
    use windows::{
        core::PCWSTR,
        Win32::Security::{
            SetFileSecurityW, DACL_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION,
        },
    };

    let descriptor = PrivateSecurityDescriptor::new(PRIVATE_FILE_SDDL)?;
    let path = wide(path.as_os_str());
    let information = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    if !unsafe { SetFileSecurityW(PCWSTR(path.as_ptr()), information, descriptor.0) }.as_bool() {
        return Err(PlatformError::Failed(format!(
            "设置私密文件 ACL 失败: {}",
            windows::core::Error::from_win32()
        )));
    }
    Ok(())
}

#[cfg(windows)]
struct PrivateSecurityDescriptor(PSECURITY_DESCRIPTOR);

#[cfg(windows)]
impl PrivateSecurityDescriptor {
    fn new(sddl: &str) -> Result<Self, PlatformError> {
        use windows::{
            core::PCWSTR,
            Win32::Security::{
                Authorization::{
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
                },
                PSECURITY_DESCRIPTOR,
            },
        };
        let sddl = wide(sddl.as_ref());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .map_err(|error| PlatformError::Failed(format!("解析私密文件 ACL 失败: {error}")))?;
        Ok(Self(descriptor))
    }
}

#[cfg(windows)]
impl Drop for PrivateSecurityDescriptor {
    fn drop(&mut self) {
        use windows::Win32::Foundation::{LocalFree, HLOCAL};
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0 .0)));
        }
    }
}

#[cfg(windows)]
fn replace_file(temporary: &Path, target: &Path) -> Result<(), PlatformError> {
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };

    let temporary = wide(temporary.as_os_str());
    let target = wide(target.as_os_str());
    unsafe {
        MoveFileExW(
            PCWSTR(temporary.as_ptr()),
            PCWSTR(target.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .map_err(|error| PlatformError::Failed(format!("发布私密文件失败: {error}")))
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, target: &Path) -> Result<(), PlatformError> {
    fs::rename(temporary, target).map_err(io_error)
}

#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn private_file_is_written_atomically_with_restricted_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("secrets").join("device-token");
        super::write_private_file(&path, b"token").unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), b"token");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let directory_mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(directory_mode, 0o700);
    }

    #[cfg(windows)]
    #[test]
    fn private_acl_descriptor_is_valid() {
        let descriptor = super::PrivateSecurityDescriptor::new(super::PRIVATE_FILE_SDDL).unwrap();
        assert!(!descriptor.0 .0.is_null());
    }
}
