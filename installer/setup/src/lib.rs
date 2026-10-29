use std::{
    fs::{self, File},
    path::Path,
};

use anyhow::{bail, Context, Result};
use serde::Serialize;

#[derive(Serialize)]
struct ServiceConfig<'a> {
    database: &'a Path,
    server_url: &'a str,
}

/// 校验安装输入并原子写入服务配置。
pub fn write_service_config(
    config_path: &Path,
    database_path: &Path,
    server_address: &str,
    server_port: &str,
) -> Result<()> {
    let server_url = build_server_url(server_address, server_port)?;
    let parent = config_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow::anyhow!("配置文件路径必须包含父目录"))?;
    fs::create_dir_all(parent).with_context(|| format!("无法创建配置目录 {}", parent.display()))?;

    let content = toml::to_string(&ServiceConfig {
        database: database_path,
        server_url: &server_url,
    })?;
    let temporary_path = config_path.with_extension(format!(
        "{}.tmp",
        config_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
    ));
    fs::write(&temporary_path, content)
        .with_context(|| format!("无法写入临时配置 {}", temporary_path.display()))?;
    File::options()
        .write(true)
        .open(&temporary_path)
        .and_then(|file| file.sync_all())
        .with_context(|| format!("无法同步临时配置 {}", temporary_path.display()))?;
    if let Err(error) = replace_file(&temporary_path, config_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    Ok(())
}

fn build_server_url(server_address: &str, server_port: &str) -> Result<String> {
    let address = server_address.trim();
    if address.is_empty() || address.contains(['\r', '\n']) {
        bail!("服务器地址不能为空且不得包含换行符");
    }
    let port = server_port
        .trim()
        .parse::<u16>()
        .context("服务器端口必须是 1-65535 的数字")?;
    if port == 0 {
        bail!("服务器端口必须在 1-65535 之间");
    }

    let candidate = if address.contains("://") {
        address.to_owned()
    } else {
        format!("https://{address}")
    };
    let mut url = url::Url::parse(&candidate).context("服务器地址格式无效")?;
    if url.scheme() != "https" && url.scheme() != "http" {
        bail!("服务器地址必须使用 HTTPS");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("服务器地址不能包含用户名或密码");
    }
    if url.port().is_some() {
        bail!("服务器地址不要包含端口，请使用单独的端口输入");
    }
    if url.query().is_some() || url.fragment().is_some() {
        bail!("服务器地址不能包含查询参数或片段");
    }
    if !matches!(url.path(), "" | "/") {
        bail!("服务器地址不能包含路径");
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("服务器地址缺少主机名"))?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if url.scheme() == "http" && !loopback {
        bail!("非回环服务器地址必须使用 HTTPS");
    }

    url.set_port(Some(port))
        .map_err(|_| anyhow::anyhow!("无法设置服务器端口"))?;
    url.set_path("");
    Ok(url.to_string().trim_end_matches('/').to_owned())
}

#[cfg(not(windows))]
fn replace_file(temporary_path: &Path, config_path: &Path) -> Result<()> {
    fs::rename(temporary_path, config_path).with_context(|| {
        format!(
            "无法将临时配置 {} 替换为 {}",
            temporary_path.display(),
            config_path.display()
        )
    })
}

#[cfg(windows)]
fn replace_file(temporary_path: &Path, config_path: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;

    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };

    let temporary: Vec<u16> = temporary_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let config: Vec<u16> = config_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MoveFileExW(
            PCWSTR(temporary.as_ptr()),
            PCWSTR(config.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    }
    .with_context(|| {
        format!(
            "无法将临时配置 {} 替换为 {}",
            temporary_path.display(),
            config_path.display()
        )
    })
}
