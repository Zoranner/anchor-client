//! 终端与 Anchor Guard 服务端的注册、状态和心跳同步。

use std::time::Duration;

use anchor_protocol::{
    ApiError, DeviceStatusResponse, HeartbeatRequest, RegisterDeviceRequest,
    RegisterDeviceResponse, PROTOCOL_VERSION,
};
use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use thiserror::Error;
use ureq::{http::Response, Agent, Body};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerSyncConfig {
    pub base_url: String,
}

impl ServerSyncConfig {
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_owned();
        if base_url.is_empty() {
            bail!("服务端地址不能为空");
        }
        if base_url.len() > 2048 {
            bail!("服务端地址过长");
        }
        if !(base_url.starts_with("https://") || base_url.starts_with("http://")) {
            bail!("服务端地址必须使用 http:// 或 https://");
        }
        let parsed = url::Url::parse(&base_url).context("服务端地址格式无效")?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            bail!("服务端地址不能包含用户名或密码");
        }
        if parsed.query().is_some() || parsed.fragment().is_some() {
            bail!("服务端地址不能包含查询参数或片段");
        }
        let host = parsed.host_str().unwrap_or_default();
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if parsed.scheme() == "http" && !loopback {
            bail!("非回环地址必须使用 HTTPS");
        }
        Ok(Self { base_url })
    }
}

#[derive(Debug, Error)]
pub enum ServerSyncError {
    #[error("服务端连接失败: {0}")]
    Transport(String),
    #[error("服务端响应无效: {0}")]
    InvalidResponse(String),
    #[error("服务端返回 HTTP {status}: {message}")]
    Api {
        status: u16,
        code: anchor_protocol::ApiErrorCode,
        message: String,
    },
}

#[derive(Clone)]
pub struct ServerSync {
    agent: Agent,
    config: ServerSyncConfig,
}

impl ServerSync {
    pub fn new(config: ServerSyncConfig) -> Self {
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .user_agent(concat!("AnchorGuard/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { agent, config }
    }

    pub fn base_url(&self) -> &str {
        &self.config.base_url
    }

    pub fn register(
        &self,
        request: &RegisterDeviceRequest,
    ) -> std::result::Result<RegisterDeviceResponse, ServerSyncError> {
        let response = self
            .agent
            .post(&self.endpoint("/api/device/register"))
            .header("X-Anchor-Protocol", &PROTOCOL_VERSION.to_string())
            .send_json(request)
            .map_err(transport_error)?;
        parse_json_response(response)
    }

    pub fn status(
        &self,
        device_token: &str,
    ) -> std::result::Result<DeviceStatusResponse, ServerSyncError> {
        let response = self
            .agent
            .get(&self.endpoint("/api/device/status"))
            .header("X-Anchor-Protocol", &PROTOCOL_VERSION.to_string())
            .header("Authorization", &format!("Bearer {device_token}"))
            .call()
            .map_err(transport_error)?;
        parse_json_response(response)
    }

    pub fn heartbeat(
        &self,
        device_token: &str,
        request: &HeartbeatRequest,
    ) -> std::result::Result<(), ServerSyncError> {
        let response = self
            .agent
            .post(&self.endpoint("/api/device/heartbeat"))
            .header("X-Anchor-Protocol", &PROTOCOL_VERSION.to_string())
            .header("Authorization", &format!("Bearer {device_token}"))
            .send_json(request)
            .map_err(transport_error)?;
        let status = response.status().as_u16();
        let body = read_body(response)?;
        if !(200..=299).contains(&status) {
            return Err(api_error(status, &body));
        }
        Ok(())
    }

    fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.config.base_url, path)
    }
}

fn parse_json_response<T: DeserializeOwned>(
    mut response: Response<Body>,
) -> std::result::Result<T, ServerSyncError> {
    let status = response.status().as_u16();
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error| ServerSyncError::Transport(format!("读取响应失败: {error}")))?;
    if !(200..=299).contains(&status) {
        return Err(api_error(status, &body));
    }
    serde_json::from_str(&body)
        .map_err(|error| ServerSyncError::InvalidResponse(format!("JSON 解析失败: {error}")))
}

fn read_body(mut response: Response<Body>) -> std::result::Result<String, ServerSyncError> {
    response
        .body_mut()
        .read_to_string()
        .map_err(|error| ServerSyncError::Transport(format!("读取响应失败: {error}")))
}

fn api_error(status: u16, body: &str) -> ServerSyncError {
    #[derive(serde::Deserialize)]
    struct ErrorEnvelope {
        error: ApiError,
    }

    match serde_json::from_str::<ErrorEnvelope>(body) {
        Ok(envelope) => ServerSyncError::Api {
            status,
            code: envelope.error.code,
            message: envelope.error.message,
        },
        Err(error) => {
            ServerSyncError::InvalidResponse(format!("HTTP {status} 响应不是标准错误结构: {error}"))
        }
    }
}

fn transport_error(error: ureq::Error) -> ServerSyncError {
    ServerSyncError::Transport(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use anchor_protocol::{
        ComputerInventory, CpuInventory, DeviceStatus, HardwareInventory, OperatingSystemInventory,
        RegisterDeviceRequest, HARDWARE_INVENTORY_SCHEMA_VERSION,
    };
    use chrono::{TimeZone, Utc};

    use super::*;

    struct OneShotServer {
        base_url: String,
        handle: thread::JoinHandle<()>,
    }

    impl OneShotServer {
        fn start<F>(status: &'static str, body: &'static str, verify: F) -> Self
        where
            F: FnOnce(&str) + Send + 'static,
        {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let handle = thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                let (mut stream, _) = loop {
                    match listener.accept() {
                        Ok(value) => break value,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(Instant::now() < deadline, "等待测试请求超时");
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => panic!("接受测试请求失败: {error}"),
                    }
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let request = read_request(&mut stream);
                verify(&request);
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            });
            Self {
                base_url: format!("http://{address}"),
                handle,
            }
        }

        fn join(self) {
            self.handle.join().unwrap();
        }
    }

    fn read_request(stream: &mut std::net::TcpStream) -> String {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if let Some(header_end) = request.windows(4).position(|value| value == b"\r\n\r\n") {
                let header_end = header_end + 4;
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let chunked = headers
                    .to_ascii_lowercase()
                    .contains("transfer-encoding: chunked");
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if chunked {
                    if request[header_end..]
                        .windows(5)
                        .any(|value| value == b"0\r\n\r\n")
                    {
                        break;
                    }
                } else if request.len() >= header_end + content_length {
                    break;
                }
            }
            if request.len() > 1024 * 1024 {
                panic!("测试请求超过 1 MiB");
            }
        }
        String::from_utf8(request).unwrap()
    }

    fn inventory() -> HardwareInventory {
        HardwareInventory {
            schema_version: HARDWARE_INVENTORY_SCHEMA_VERSION,
            captured_at: Utc.with_ymd_and_hms(2026, 10, 29, 8, 0, 0).unwrap(),
            computer: ComputerInventory::default(),
            operating_system: OperatingSystemInventory {
                hostname: "test-pc".into(),
                ..Default::default()
            },
            cpu: CpuInventory {
                model: "Example CPU".into(),
                ..Default::default()
            },
            memory_modules: vec![],
            disks: vec![],
            volumes: vec![],
            network_adapters: vec![],
            usb_devices: vec![],
            optical_drives: vec![],
            gpus: vec![],
        }
    }

    fn config(server: &OneShotServer) -> ServerSyncConfig {
        ServerSyncConfig::new(server.base_url.clone()).unwrap()
    }

    #[test]
    fn server_url_enforces_https_and_loopback_http() {
        assert!(ServerSyncConfig::new("https://anchor.example.com/").is_ok());
        assert!(ServerSyncConfig::new("http://127.0.0.1:8080").is_ok());
        assert!(ServerSyncConfig::new("http://localhost:8080").is_ok());
        assert!(ServerSyncConfig::new("http://example.com").is_err());
        assert!(ServerSyncConfig::new("http://localhost.example.com").is_err());
        assert!(ServerSyncConfig::new("ftp://example.com").is_err());
    }

    #[test]
    fn register_sends_protocol_and_parses_token() {
        let server = OneShotServer::start(
            "201 Created",
            r#"{"device_id":"device-1","status":"pending","device_token":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
            |request| {
                assert!(request.starts_with("POST /api/device/register "));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("x-anchor-protocol: 1"));
                let body: serde_json::Value =
                    serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(
                    body["installation_id"],
                    "3f0f5c1e-6d1a-4dd0-9d32-9e5b2a1c8f44"
                );
            },
        );
        let sync = ServerSync::new(config(&server));
        let response = sync
            .register(&RegisterDeviceRequest {
                installation_id: "3f0f5c1e-6d1a-4dd0-9d32-9e5b2a1c8f44".into(),
                hostname: "test-pc".into(),
                client_version: "0.1.2".into(),
                inventory: inventory(),
            })
            .unwrap();
        server.join();

        assert_eq!(response.status, DeviceStatus::Pending);
        assert!(response.device_token.is_some());
    }

    #[test]
    fn status_uses_bearer_token() {
        let server = OneShotServer::start(
            "200 OK",
            r#"{"status":"active","approved_at":"2026-10-29T08:00:00Z"}"#,
            |request| {
                assert!(request.starts_with("GET /api/device/status "));
                assert!(request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer token"));
            },
        );
        let sync = ServerSync::new(config(&server));
        let response = sync.status("token").unwrap();
        server.join();

        assert_eq!(response.status, DeviceStatus::Active);
        assert!(response.approved_at.is_some());
    }

    #[test]
    fn heartbeat_accepts_204() {
        let server = OneShotServer::start("204 No Content", "", |request| {
            assert!(request.starts_with("POST /api/device/heartbeat "));
            let body: serde_json::Value =
                serde_json::from_str(request.split_once("\r\n\r\n").unwrap().1).unwrap();
            assert_eq!(body["inventory_hash"], "abc");
        });
        let sync = ServerSync::new(config(&server));
        sync.heartbeat(
            "token",
            &HeartbeatRequest {
                client_version: "0.1.2".into(),
                inventory_hash: "abc".into(),
            },
        )
        .unwrap();
        server.join();
    }

    #[test]
    fn api_errors_keep_status_and_code() {
        let server = OneShotServer::start(
            "403 Forbidden",
            r#"{"error":{"code":"device_pending","message":"pending","details":{}}}"#,
            |_| {},
        );
        let sync = ServerSync::new(config(&server));
        let error = sync.status("token").unwrap_err();
        server.join();

        let debug = format!("{error:?}");
        assert!(
            matches!(
                error,
                ServerSyncError::Api {
                    status: 403,
                    code: anchor_protocol::ApiErrorCode::DevicePending,
                    ..
                }
            ),
            "{debug}"
        );
    }
}
