# anchor-client

Windows 终端客户端仓库，包含 Windows 服务、会话协作程序、登录组件、桌面端、安装器和客户端构建入口。

## 目录

- `src/`、`Cargo.toml`：`anchor-build` 构建与打包入口。
- `service/`：Windows Service，负责策略授权、设备执行和审计。
- `session/`：交互会话协作程序。
- `credential/`：Credential Provider 与登录过滤组件。
- `desktop/`：Tauri 桌面端。
- `installer/`：WiX 安装定义和安装期组件。
- `crates/`：客户端专用 crate；`crates/shared` 是 `anchor-shared` 子模块。

## 构建与打包

```powershell
cargo run --release --locked -- release
```

产物输出到 `release/`，目前只生成 MSI。

## 验证

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```
