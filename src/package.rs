use std::{env, fs, path::Path, process::Command};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::command::{client_root, run};

const TARGET: &str = "windows-x64";

#[derive(Serialize)]
struct ManifestArtifact {
    name: String,
    sha256: String,
}

#[derive(Serialize)]
struct Manifest {
    version: String,
    revision: String,
    target: &'static str,
    artifacts: Vec<ManifestArtifact>,
}

pub fn package() -> Result<()> {
    if !cfg!(windows) {
        bail!("Windows 安装包只能在 Windows 上构建");
    }

    let root = client_root();
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let binaries = root.join("target/release");
    let installer = root.join("installer/AnchorGuard.wxs");
    let desktop_binary = root.join("desktop/src-tauri/target/release/anchor-guard.exe");
    let output = root.join("release");
    let msi = output.join(format!("anchor-guard-v{version}-windows-x64.msi"));
    let manifest_path = output.join("manifest.json");
    let checksums_path = output.join("SHA256SUMS");

    ensure_file("安装器定义", &installer)?;
    ensure_file("桌面端程序", &desktop_binary)?;

    let binary_names = [
        ("服务程序", "anchor-service.exe"),
        ("会话协作程序", "anchor-session.exe"),
        ("安装器配置程序", "anchor-setup.exe"),
        ("安装器动作 DLL", "anchor_installer_action.dll"),
        ("登录组件 DLL", "anchor_credential.dll"),
    ];
    for (label, name) in binary_names {
        ensure_file(label, &binaries.join(name))?;
    }

    for path in [&msi, &manifest_path, &checksums_path] {
        if path.exists() {
            bail!("发布文件已存在，请先移除: {}", path.display());
        }
    }

    ensure_wix()?;
    fs::create_dir_all(&output)
        .with_context(|| format!("创建发布目录失败: {}", output.display()))?;

    let mut wix = Command::new("wix");
    wix.current_dir(root)
        .arg("build")
        .args(["-ext", "WixToolset.UI.wixext"])
        .arg(&installer)
        .args(["-arch", "x64"])
        .arg("-d")
        .arg(format!("ProductVersion={version}"));
    for (variable, name) in [
        ("AnchorServiceBinary", "anchor-service.exe"),
        ("AnchorSessionBinary", "anchor-session.exe"),
        ("AnchorSetupBinary", "anchor-setup.exe"),
        ("AnchorInstallerActionDll", "anchor_installer_action.dll"),
        ("AnchorCredentialDll", "anchor_credential.dll"),
    ] {
        wix.arg("-d")
            .arg(format!("{variable}={}", binaries.join(name).display()));
    }
    wix.arg("-d")
        .arg(format!("DesktopBinary={}", desktop_binary.display()))
        .arg("-o")
        .arg(&msi);
    run(&mut wix)?;

    let wixpdb = msi.with_extension("wixpdb");
    if wixpdb.exists() {
        fs::remove_file(&wixpdb)
            .with_context(|| format!("删除 WiX 调试文件失败: {}", wixpdb.display()))?;
    }

    let msi_bytes = fs::read(&msi).with_context(|| format!("读取安装包失败: {}", msi.display()))?;
    let sha256 = hex::encode(Sha256::digest(&msi_bytes));
    let artifact_name = msi
        .file_name()
        .and_then(|name| name.to_str())
        .context("安装包文件名不是有效 UTF-8")?
        .to_owned();
    let manifest = Manifest {
        version,
        revision: revision(root),
        target: TARGET,
        artifacts: vec![ManifestArtifact {
            name: artifact_name.clone(),
            sha256: sha256.clone(),
        }],
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).context("序列化发布清单失败")?;
    fs::write(&manifest_path, format!("{manifest_json}\n"))
        .with_context(|| format!("写入发布清单失败: {}", manifest_path.display()))?;
    fs::write(&checksums_path, format!("{sha256}  {artifact_name}\n"))
        .with_context(|| format!("写入校验文件失败: {}", checksums_path.display()))?;

    eprintln!("发布物已生成: {}", output.display());
    Ok(())
}

fn ensure_file(label: &str, path: &Path) -> Result<()> {
    if !path.is_file() {
        bail!("缺少{label}: {}", path.display());
    }
    Ok(())
}

fn ensure_wix() -> Result<()> {
    let available = Command::new("wix")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !available {
        bail!("未找到 WiX 5，可执行 wix --version 检查 PATH");
    }
    Ok(())
}

fn revision(root: &Path) -> String {
    if let Ok(value) = env::var("GITHUB_SHA") {
        if !value.is_empty() {
            return value;
        }
    }
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}
