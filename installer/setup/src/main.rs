use std::{path::PathBuf, process::ExitCode};

use anchor_setup::write_service_config;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "anchor-setup", about = "Anchor Guard 安装配置工具")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 写入 Windows 服务使用的本地配置文件。
    WriteServiceConfig {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        database: PathBuf,
        #[arg(long)]
        server_address: String,
        #[arg(long, default_value = "8080")]
        server_port: String,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::WriteServiceConfig {
            config,
            database,
            server_address,
            server_port,
        } => match write_service_config(&config, &database, &server_address, &server_port) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("错误: {error:#}");
                ExitCode::FAILURE
            }
        },
    }
}
