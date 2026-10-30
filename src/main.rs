mod build;
mod command;
mod package;

use std::env;

use anyhow::{bail, Result};

const HELP: &str = "\
Anchor Guard 客户端构建与打包

用法:
  cargo run --release --locked -- <命令>

命令:
  build     构建 Rust workspace 和桌面端
  package   用已构建的程序生成 MSI、manifest.json 和 SHA256SUMS
  release   依次执行 build 和 package（默认）
";

fn main() -> Result<()> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().unwrap_or_else(|| "release".to_owned());
    if arguments.next().is_some() {
        bail!("只接受一个命令参数\n\n{HELP}");
    }

    match command.as_str() {
        "build" => build::build(),
        "package" => package::package(),
        "release" => {
            build::build()?;
            package::package()
        }
        "help" | "-h" | "--help" => {
            print!("{HELP}");
            Ok(())
        }
        _ => bail!("未知命令: {command}\n\n{HELP}"),
    }
}
