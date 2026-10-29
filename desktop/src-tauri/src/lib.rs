//! Anchor Guard 桌面端。

pub mod app;
pub mod commands;
pub mod ipc;
pub mod preferences;

pub fn run() {
    app::run();
}
