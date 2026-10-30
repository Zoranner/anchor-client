use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};

pub fn client_root() -> Result<PathBuf> {
    let mut directory = env::current_dir().context("读取当前目录失败")?;
    loop {
        if is_client_root(&directory) {
            return Ok(directory);
        }
        if !directory.pop() {
            bail!("无法定位客户端仓库根目录");
        }
    }
}

fn is_client_root(directory: &Path) -> bool {
    directory.join("Cargo.toml").is_file()
        && directory.join("desktop").is_dir()
        && directory.join("installer").is_dir()
}

pub fn run(command: &mut Command) -> Result<()> {
    let display = format!("{command:?}");
    eprintln!("> {display}");
    let status = command
        .status()
        .with_context(|| format!("启动命令失败: {display}"))?;
    if !status.success() {
        bail!("命令失败（{status}）: {display}");
    }
    Ok(())
}
