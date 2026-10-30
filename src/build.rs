use std::{path::Path, process::Command};

use anyhow::Result;

use crate::command::{client_root, run};

pub fn build() -> Result<()> {
    let root = client_root()?;
    let mut cargo = Command::new("cargo");
    cargo.current_dir(&root).args([
        "build",
        "--release",
        "--locked",
        "--workspace",
        "--exclude",
        "anchor-build",
    ]);
    run(&mut cargo)?;

    build_desktop(&root)
}

fn build_desktop(root: &Path) -> Result<()> {
    let desktop = root.join("desktop");
    let mut install = Command::new("bun");
    install
        .current_dir(&desktop)
        .args(["install", "--frozen-lockfile"]);
    run(&mut install)?;

    let mut build = Command::new("bun");
    build
        .current_dir(&desktop)
        .args(["run", "tauri", "build", "--no-bundle"]);
    run(&mut build)
}
