use std::{fs, path::Path, process::Command};

use anyhow::{bail, Context, Result};

use crate::command::{client_root, run};

pub fn package() -> Result<()> {
    if !cfg!(windows) {
        bail!("Windows 安装包只能在 Windows 上构建");
    }

    let root = client_root()?;
    let version = env!("CARGO_PKG_VERSION").to_owned();
    let binaries = root.join("target/release");
    let installer = root.join("installer/AnchorGuard.wxs");
    let desktop_binary = root.join("desktop/src-tauri/target/release/anchor-guard.exe");
    let output = root.join("release");
    let msi = output.join(format!("anchor-guard-v{version}-windows-x64.msi"));

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

    if msi.exists() {
        bail!("发布文件已存在，请先移除: {}", msi.display());
    }

    ensure_wix()?;
    fs::create_dir_all(&output)
        .with_context(|| format!("创建发布目录失败: {}", output.display()))?;

    let mut wix = Command::new("wix");
    wix.current_dir(&root)
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

    ensure_file("安装包", &msi)?;

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
