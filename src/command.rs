use std::{path::Path, process::Command};

use anyhow::{bail, Context, Result};

pub fn client_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
