//! 服务端模式下的注册、审批轮询、心跳和策略同步。

use std::{
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use anchor_core::{crypto, PolicyPackage, PolicyPayload};
use anchor_ipc::{ServerStatusView, ServerSyncState};
use anchor_protocol::{
    ApiErrorCode, DeviceStatus, HeartbeatRequest, PolicyEnvelope, RegisterDeviceRequest, ServerKey,
    ServerKeyStatus, ServerKeyringResponse,
};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;

use crate::{
    device_identity::{DeviceTokenStore, InstallationIdentity},
    inventory_report::InventoryReport,
    server_sync::{ServerSync, ServerSyncConfig, ServerSyncError},
    Service,
};

const INVENTORY_REFRESH_INTERVAL: Duration = Duration::from_secs(10 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub status: DeviceStatus,
    pub policy_version: Option<u64>,
}

pub struct ServerRuntime {
    service: Arc<Service>,
    sync: ServerSync,
    identity: InstallationIdentity,
    token_store: DeviceTokenStore,
    inventory: Mutex<Option<InventoryRefresh>>,
    status: Arc<RwLock<Option<ServerStatusView>>>,
}

struct InventoryRefresh {
    report: InventoryReport,
    refreshed_at: Instant,
}

impl ServerRuntime {
    pub fn new(
        service: Arc<Service>,
        server_url: impl Into<String>,
        identity: InstallationIdentity,
        device_token_path: std::path::PathBuf,
        status: Arc<RwLock<Option<ServerStatusView>>>,
    ) -> Result<Self> {
        let sync = ServerSync::new(ServerSyncConfig::new(server_url)?);
        if let Some(trust) = service.store.server_trust()? {
            let key = crypto::verifying_key_from_hex(&trust.public_key_hex)
                .context("已保存的服务端公钥无效")?;
            validate_key_id(&trust.key_id, &key)?;
            service.set_verifying_key(key);
        }
        *status
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ServerStatusView::default());
        Ok(Self {
            service,
            sync,
            identity,
            token_store: DeviceTokenStore::new(device_token_path),
            inventory: Mutex::new(None),
            status,
        })
    }

    pub fn installation_id(&self) -> &InstallationIdentity {
        &self.identity
    }

    pub fn sync_once(&self, now: DateTime<Utc>) -> Result<SyncReport> {
        let result = self.sync_once_inner(now);
        match &result {
            Ok(report) => self.record_status_success(report, now),
            Err(error) => self.record_status_error(error),
        }
        result
    }

    fn sync_once_inner(&self, now: DateTime<Utc>) -> Result<SyncReport> {
        let Some(token) = self.token_store.load()? else {
            return self.register(now);
        };

        let status = match self.sync.status(&token) {
            Ok(response) => response.status,
            Err(error) if is_token_invalid(&error) => {
                self.token_store.clear()?;
                return self.register(now);
            }
            Err(error) => return Err(anyhow::Error::new(error).context("查询设备审批状态失败")),
        };
        if status != DeviceStatus::Active {
            return Ok(SyncReport {
                status,
                policy_version: None,
            });
        }

        let keyring = self
            .sync
            .keyring(&token)
            .map_err(anyhow::Error::new)
            .context("获取服务端密钥环失败")?;
        let active = set_active_trust(&self.service, &keyring)?;

        let report = self.current_inventory(false)?;
        let mut heartbeat = HeartbeatRequest {
            client_version: env!("CARGO_PKG_VERSION").into(),
            inventory_hash: report.hash_hex.clone(),
            inventory: None,
        };
        if let Err(error) = self.sync.heartbeat(&token, &heartbeat) {
            if is_token_invalid(&error) {
                self.token_store.clear()?;
                return self.register(now);
            }
            if !is_inventory_changed(&error) {
                return Err(anyhow::Error::new(error).context("发送设备心跳失败"));
            }
            let refreshed = self.current_inventory(true)?;
            heartbeat.inventory_hash = refreshed.hash_hex.clone();
            heartbeat.inventory = Some(refreshed.inventory.clone());
            self.sync
                .heartbeat(&token, &heartbeat)
                .map_err(anyhow::Error::new)
                .context("上报变化后的硬件清单失败")?;
        }

        let policy = self
            .sync
            .fetch_policy(&token)
            .map_err(anyhow::Error::new)
            .context("获取服务端策略失败")?;
        let current_version = self.service.active_policy_version()?;
        if let Some(envelope) = policy {
            if current_version.is_none_or(|version| envelope.version > version) {
                self.import_policy(&keyring, &active, &envelope, now)?;
            }
        }

        Ok(SyncReport {
            status: DeviceStatus::Active,
            policy_version: self.service.active_policy_version()?.or(current_version),
        })
    }

    fn record_status_success(&self, report: &SyncReport, now: DateTime<Utc>) {
        let policy_version = report
            .policy_version
            .or_else(|| self.service.active_policy_version().ok().flatten());
        self.write_status(ServerStatusView {
            state: sync_state(report.status),
            last_sync_at: Some(now),
            last_error: None,
            policy_version,
        });
    }

    fn record_status_error(&self, error: &anyhow::Error) {
        let previous = self.status_snapshot();
        let policy_version = previous
            .as_ref()
            .and_then(|status| status.policy_version)
            .or_else(|| self.service.active_policy_version().ok().flatten());
        self.write_status(ServerStatusView {
            state: if is_server_unreachable(error) {
                ServerSyncState::Unreachable
            } else {
                ServerSyncState::Error
            },
            last_sync_at: previous.and_then(|status| status.last_sync_at),
            last_error: Some(truncate_error(&format!("{error:#}"))),
            policy_version,
        });
    }

    fn status_snapshot(&self) -> Option<ServerStatusView> {
        self.status
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn write_status(&self, status: ServerStatusView) {
        *self
            .status
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(status);
    }

    fn register(&self, _now: DateTime<Utc>) -> Result<SyncReport> {
        let report = self.current_inventory(true)?;
        let hostname = hostname(&report);
        let response = self
            .sync
            .register(&RegisterDeviceRequest {
                installation_id: self.identity.as_string(),
                hostname,
                client_version: env!("CARGO_PKG_VERSION").into(),
                inventory: report.inventory.clone(),
            })
            .map_err(anyhow::Error::new)
            .context("注册设备失败")?;
        let Some(token) = response.device_token else {
            bail!("服务端没有返回设备令牌");
        };
        self.token_store.save(&token)?;
        Ok(SyncReport {
            status: response.status,
            policy_version: None,
        })
    }

    fn current_inventory(&self, force_refresh: bool) -> Result<InventoryReport> {
        let mut inventory = self
            .inventory
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let needs_refresh = force_refresh
            || inventory
                .as_ref()
                .is_none_or(|cached| cached.refreshed_at.elapsed() >= INVENTORY_REFRESH_INTERVAL);
        if needs_refresh {
            *inventory = Some(InventoryRefresh {
                report: InventoryReport::collect()?,
                refreshed_at: Instant::now(),
            });
        }
        Ok(inventory
            .as_ref()
            .expect("inventory refresh always populates the cache")
            .report
            .clone())
    }

    fn import_policy(
        &self,
        keyring: &ServerKeyringResponse,
        active: &ServerKey,
        envelope: &PolicyEnvelope,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let key = keyring
            .keys
            .iter()
            .find(|key| key.key_id == envelope.server_key_id)
            .ok_or_else(|| anyhow::anyhow!("策略签名密钥不在服务端密钥环中"))?;
        let verifying_key = parse_key(key)?;
        let active_key = parse_key(active)?;
        let payload: PolicyPayload = serde_json::from_value(envelope.policy_json.clone())
            .context("解析服务端策略载荷失败")?;
        let package = PolicyPackage {
            payload,
            signature: envelope.signature_hex.clone(),
        };
        let package_json = serde_json::to_string(&package)?;

        self.service.set_verifying_key(verifying_key);
        let outcome = self
            .service
            .import_policy_json(&package_json, "AnchorGuard", now);
        self.service.set_verifying_key(active_key);
        let outcome = outcome.map_err(|error| {
            anyhow::anyhow!("导入服务端策略失败: {} ({})", error.message, error.code)
        })?;
        if !outcome.accepted {
            bail!("服务端策略未通过本地校验: {}", outcome.reason);
        }
        Ok(())
    }
}

fn set_active_trust(service: &Service, keyring: &ServerKeyringResponse) -> Result<ServerKey> {
    let active = keyring
        .keys
        .iter()
        .find(|key| key.status == ServerKeyStatus::Active)
        .ok_or_else(|| anyhow::anyhow!("服务端密钥环没有活动密钥"))?;
    let key = parse_key(active)?;
    let changed = service
        .store
        .server_trust()?
        .is_none_or(|trust| trust.key_id != active.key_id);
    service.set_verifying_key(key);
    if changed {
        service.store.set_server_trust(
            &anchor_store::ServerTrust {
                key_id: active.key_id.clone(),
                public_key_hex: active.public_key_hex.clone(),
            },
            Utc::now(),
        )?;
    }
    Ok(active.clone())
}

fn parse_key(key: &ServerKey) -> Result<VerifyingKey> {
    let verifying_key =
        crypto::verifying_key_from_hex(&key.public_key_hex).context("服务端公钥无效")?;
    validate_key_id(&key.key_id, &verifying_key)?;
    Ok(verifying_key)
}

fn validate_key_id(key_id: &str, key: &VerifyingKey) -> Result<()> {
    if crypto::sha256_hex(&key.to_bytes()) != key_id {
        bail!("服务端 key_id 与公钥不匹配");
    }
    Ok(())
}

fn hostname(report: &InventoryReport) -> String {
    let hostname = report.inventory.operating_system.hostname.trim();
    if !hostname.is_empty() {
        return hostname.into();
    }
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown-host".into())
}

fn is_token_invalid(error: &ServerSyncError) -> bool {
    matches!(
        error,
        ServerSyncError::Api {
            code: ApiErrorCode::TokenInvalid,
            ..
        }
    )
}

fn is_inventory_changed(error: &ServerSyncError) -> bool {
    matches!(
        error,
        ServerSyncError::Api {
            code: ApiErrorCode::InventoryChanged,
            ..
        }
    )
}

fn sync_state(status: DeviceStatus) -> ServerSyncState {
    match status {
        DeviceStatus::Pending => ServerSyncState::Pending,
        DeviceStatus::Active => ServerSyncState::Active,
        DeviceStatus::Rejected => ServerSyncState::Rejected,
        DeviceStatus::Disabled => ServerSyncState::Disabled,
    }
}

fn is_server_unreachable(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<ServerSyncError>(),
            Some(ServerSyncError::Transport(_) | ServerSyncError::InvalidResponse(_))
        )
    })
}

fn truncate_error(message: &str) -> String {
    const MAX_CHARS: usize = 500;
    let mut chars = message.chars();
    let truncated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_id_must_match_the_public_key() {
        let (_, key) = crypto::generate_keypair();
        let key_id = crypto::sha256_hex(&key.to_bytes());
        assert!(validate_key_id(&key_id, &key).is_ok());
        assert!(validate_key_id(&"0".repeat(64), &key).is_err());
    }

    #[test]
    fn sync_error_classification_matches_protocol_codes() {
        let token_invalid = ServerSyncError::Api {
            status: 401,
            code: ApiErrorCode::TokenInvalid,
            message: "invalid".into(),
        };
        let inventory_changed = ServerSyncError::Api {
            status: 409,
            code: ApiErrorCode::InventoryChanged,
            message: "changed".into(),
        };
        assert!(is_token_invalid(&token_invalid));
        assert!(!is_inventory_changed(&token_invalid));
        assert!(is_inventory_changed(&inventory_changed));
        assert!(!is_token_invalid(&inventory_changed));
    }

    #[test]
    fn status_mapping_matches_device_states() {
        assert_eq!(sync_state(DeviceStatus::Pending), ServerSyncState::Pending);
        assert_eq!(sync_state(DeviceStatus::Active), ServerSyncState::Active);
        assert_eq!(
            sync_state(DeviceStatus::Rejected),
            ServerSyncState::Rejected
        );
        assert_eq!(
            sync_state(DeviceStatus::Disabled),
            ServerSyncState::Disabled
        );
    }

    #[test]
    fn transport_errors_are_unreachable_but_api_errors_are_not() {
        let transport = anyhow::Error::new(ServerSyncError::Transport("offline".into()));
        assert!(is_server_unreachable(&transport));

        let api = anyhow::Error::new(ServerSyncError::Api {
            status: 500,
            code: ApiErrorCode::Internal,
            message: "failed".into(),
        });
        assert!(!is_server_unreachable(&api));
    }
}
