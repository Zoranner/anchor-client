//! Anchor Guard 终端守护服务。

pub mod config;
pub mod device_identity;
pub mod enforce;
pub mod inventory_report;
pub mod scan_runtime;
pub mod server_runtime;
pub mod server_sync;

pub use authority::Service;

mod authority;
