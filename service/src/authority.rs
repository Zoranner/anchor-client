use std::fs;
use std::path::Path;
use std::sync::RwLock;

use anchor_core::{
    decide_import, evaluate_authorization, AuditPayload, AuthorizationDenial, AuthorizationState,
    EventType, ImportDecision,
};
use anchor_ipc::{
    AuditFilter, AuditPage, AuditViewEvent, AuthorizationResponse, DesktopProtocolError,
    DesktopRequest, DesktopResponse, DesktopStatus, ImportResponse, PolicyInspection,
    PolicySummary, ServiceRequest, ServiceResponse, MAX_POLICY_PACKAGE_BYTES,
};
use anchor_store::{EventFilter, Store};
use anchor_system::{
    error::PlatformError,
    pipe::{
        serve_desktop_once, serve_once, serve_session_status_once, PipeServeOutcome,
        DEFAULT_CONTROL_PIPE,
    },
    policy_media::policy_media_roots,
    session::active_session_operator,
};
use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;

const POLICY_FILE_NAME: &str = "anchor-guard.policy";

/// 服务层对一次导入请求的结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    pub accepted: bool,
    pub reason: String,
}

/// 本地授权的唯一组织者。
pub struct Service {
    pub(crate) store: Store,
    verifying_key: RwLock<Option<VerifyingKey>>,
    pub(crate) device_id: String,
}

impl Service {
    pub fn new(store: Store, verifying_key: VerifyingKey, device_id: impl Into<String>) -> Self {
        Self::with_optional_key(store, Some(verifying_key), device_id)
    }

    pub fn with_optional_key(
        store: Store,
        verifying_key: Option<VerifyingKey>,
        device_id: impl Into<String>,
    ) -> Self {
        Self {
            store,
            verifying_key: RwLock::new(verifying_key),
            device_id: device_id.into(),
        }
    }

    pub fn set_verifying_key(&self, verifying_key: VerifyingKey) {
        *self
            .verifying_key
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(verifying_key);
    }

    pub fn verifying_key(&self) -> Option<VerifyingKey> {
        *self
            .verifying_key
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn active_policy_version(&self) -> Result<Option<u64>, anchor_store::StoreError> {
        Ok(self
            .store
            .active_policy()?
            .map(|package| package.payload.version))
    }

    /// 返回 Windows 登录和设备执行共用的当前授权结论。
    /// 存储不可读、链异常或运行状态无法解析时一律拒绝。
    pub fn authorization_state(&self, now: DateTime<Utc>) -> AuthorizationState {
        self.authorization_state_with_enforcement_failure(now, true)
    }

    /// 记录用户会话协作程序故障并进入运行期 fail-closed 状态。
    pub fn record_session_assistant_failure(
        &self,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(), anchor_store::StoreError> {
        let mut event = AuditPayload::new(
            &self.device_id,
            "AnchorGuard",
            EventType::SessionAssistantFailed,
            "session_assistant",
            "failed",
        );
        event.timestamp = now;
        event.resource = Some(reason.into());
        self.store
            .append_audit_and_set_enforcement_failure(event, "session_assistant_failed", now)
    }

    /// 记录服务启动、审计校验与停止等生命周期事件。
    pub fn record_lifecycle_event(
        &self,
        event_type: EventType,
        action: &str,
        result: &str,
        now: DateTime<Utc>,
    ) -> Result<(), anchor_store::StoreError> {
        let mut event =
            AuditPayload::new(&self.device_id, "AnchorGuard", event_type, action, result);
        event.timestamp = now;
        self.store.append_audit(event)
    }

    /// 返回本地审计链是否完整。
    pub fn audit_chain_healthy(&self) -> Result<bool, anchor_store::StoreError> {
        self.store.audit_chain_healthy()
    }

    /// 返回桌面端展示的授权、策略摘要和最后可信时间。
    pub fn desktop_status(
        &self,
        now: DateTime<Utc>,
    ) -> Result<DesktopStatus, DesktopProtocolError> {
        let policy = self
            .current_policy_summary()
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        let last_trusted_at = self
            .store
            .last_trusted_at()
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        Ok(DesktopStatus {
            authorization: authorization_response(self.authorization_state(now)),
            policy,
            last_trusted_at,
        })
    }

    /// 只校验候选策略，不写策略库、不写审计、不更新可信时间。
    pub fn inspect_policy(
        &self,
        package_json: &str,
        now: DateTime<Utc>,
    ) -> Result<PolicyInspection, DesktopProtocolError> {
        let current = self
            .current_policy_summary()
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        if package_json.len() > MAX_POLICY_PACKAGE_BYTES {
            return Ok(PolicyInspection {
                accepted: false,
                reason: "policy_too_large".into(),
                current,
                candidate: None,
            });
        }
        let package = match serde_json::from_str::<anchor_core::PolicyPackage>(package_json) {
            Ok(package) => package,
            Err(_) => {
                return Ok(PolicyInspection {
                    accepted: false,
                    reason: "policy_parse_failed".into(),
                    current,
                    candidate: None,
                })
            }
        };
        let current_version = current.as_ref().map(|summary| summary.payload.version);
        let Some(verifying_key) = self.verifying_key() else {
            return Ok(PolicyInspection {
                accepted: false,
                reason: "verifying_key_unavailable".into(),
                current,
                candidate: Some(PolicySummary {
                    imported_at: None,
                    payload: package.payload,
                }),
            });
        };
        let decision = decide_import(&package, &verifying_key, current_version, now);
        Ok(PolicyInspection {
            accepted: decision == ImportDecision::Accept,
            reason: import_rejection_reason(decision).into(),
            current,
            candidate: Some(PolicySummary {
                imported_at: None,
                payload: package.payload,
            }),
        })
    }

    /// 从桌面端提交的 JSON 导入策略；校验和原子替换由服务完成。
    pub fn import_policy_json(
        &self,
        package_json: &str,
        user: &str,
        now: DateTime<Utc>,
    ) -> Result<ImportResponse, DesktopProtocolError> {
        if package_json.len() > MAX_POLICY_PACKAGE_BYTES {
            self.append_import_rejection(user, now, "policy_too_large")
                .map_err(|error| {
                    DesktopProtocolError::new("store_unavailable", error.to_string())
                })?;
            return Ok(ImportResponse {
                accepted: false,
                reason: "policy_too_large".into(),
            });
        }
        let package = match serde_json::from_str::<anchor_core::PolicyPackage>(package_json) {
            Ok(package) => package,
            Err(_) => {
                self.append_import_rejection(user, now, "policy_parse_failed")
                    .map_err(|error| {
                        DesktopProtocolError::new("store_unavailable", error.to_string())
                    })?;
                return Ok(ImportResponse {
                    accepted: false,
                    reason: "policy_parse_failed".into(),
                });
            }
        };
        let current_version = self
            .store
            .active_policy()
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?
            .map(|package| package.payload.version);
        let Some(verifying_key) = self.verifying_key() else {
            self.append_import_rejection(user, now, "verifying_key_unavailable")
                .map_err(|error| {
                    DesktopProtocolError::new("store_unavailable", error.to_string())
                })?;
            return Ok(ImportResponse {
                accepted: false,
                reason: "verifying_key_unavailable".into(),
            });
        };
        let decision = decide_import(&package, &verifying_key, current_version, now);
        self.store
            .import_policy(&package, decision, &self.device_id, user, now)
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        Ok(ImportResponse {
            accepted: decision == ImportDecision::Accept,
            reason: import_rejection_reason(decision).into(),
        })
    }

    /// 只读查询在线审计页。
    pub fn query_audit(
        &self,
        filter: AuditFilter,
        after_sequence: Option<i64>,
        limit: u32,
        _now: DateTime<Utc>,
    ) -> Result<AuditPage, DesktopProtocolError> {
        let event_type = filter
            .event_type
            .map(|value| {
                value.parse::<EventType>().map_err(|_| {
                    DesktopProtocolError::new("invalid_event_type", "未知审计事件类型")
                })
            })
            .transpose()?;
        let store_filter = EventFilter {
            since: filter.since,
            until: filter.until,
            event_type,
            user: filter.user,
            device_id: None,
            resource_prefix: filter.resource_prefix,
            policy_version: filter.policy_version,
            limit: None,
        };
        let page = self
            .store
            .query_audit_page(&store_filter, after_sequence, limit)
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        let next_after_sequence = page.last().map(|(sequence, _)| *sequence);
        let events = page
            .into_iter()
            .map(|(sequence, event)| AuditViewEvent { sequence, event })
            .collect::<Vec<_>>();
        let chain_healthy = self
            .store
            .audit_chain_healthy()
            .map_err(|error| DesktopProtocolError::new("store_unavailable", error.to_string()))?;
        Ok(AuditPage {
            events,
            next_after_sequence,
            chain_healthy,
        })
    }

    /// 处理桌面端管道的一条请求。
    pub fn handle_desktop_request(
        &self,
        request: DesktopRequest,
        user: &str,
        now: DateTime<Utc>,
    ) -> DesktopResponse {
        match request {
            DesktopRequest::GetStatus => match self.desktop_status(now) {
                Ok(status) => DesktopResponse::Status(status),
                Err(error) => DesktopResponse::Error(error),
            },
            DesktopRequest::InspectPolicy { package_json } => {
                match self.inspect_policy(&package_json, now) {
                    Ok(inspection) => DesktopResponse::Inspection(inspection),
                    Err(error) => DesktopResponse::Error(error),
                }
            }
            DesktopRequest::ImportPolicy { package_json } => {
                match self.import_policy_json(&package_json, user, now) {
                    Ok(response) => DesktopResponse::Import(response),
                    Err(error) => DesktopResponse::Error(error),
                }
            }
            DesktopRequest::QueryAudit {
                filter,
                after_sequence,
                limit,
            } => match self.query_audit(filter, after_sequence, limit, now) {
                Ok(page) => DesktopResponse::Audit(page),
                Err(error) => DesktopResponse::Error(error),
            },
        }
    }

    /// 在桌面端管道服务一条请求；操作者由当前活动会话决定。
    pub fn serve_desktop_pipe_once(
        &self,
        now: DateTime<Utc>,
    ) -> Result<PipeServeOutcome, PlatformError> {
        let user = active_session_operator()
            .map(|operator| operator.user)
            .unwrap_or_else(|| "unknown".into());
        serve_desktop_once(|request| {
            let request: DesktopRequest = serde_json::from_slice(request)
                .map_err(|error| PlatformError::Failed(format!("桌面端 JSON 协议错误: {error}")))?;
            serde_json::to_vec(&self.handle_desktop_request(request, &user, now))
                .map_err(|error| PlatformError::Failed(format!("桌面端 JSON 编码错误: {error}")))
        })
    }

    /// 记录桌面端启动失败；不改变授权结论。
    pub fn record_desktop_launch_failure(
        &self,
        reason: &str,
        now: DateTime<Utc>,
    ) -> Result<(), anchor_store::StoreError> {
        let mut event = AuditPayload::new(
            &self.device_id,
            "AnchorGuard",
            EventType::DesktopLaunchFailed,
            "desktop_launch",
            "failed",
        );
        event.timestamp = now;
        event.resource = Some(reason.into());
        self.store.append_audit(event)
    }

    fn current_policy_summary(&self) -> Result<Option<PolicySummary>, anchor_store::StoreError> {
        Ok(self
            .store
            .active_policy_record()?
            .map(|record| PolicySummary {
                imported_at: Some(record.imported_at),
                payload: record.package.payload,
            }))
    }

    pub(crate) fn authorization_state_for_enforcement(
        &self,
        now: DateTime<Utc>,
    ) -> AuthorizationState {
        self.authorization_state_with_enforcement_failure(now, false)
    }

    fn authorization_state_with_enforcement_failure(
        &self,
        now: DateTime<Utc>,
        check_enforcement_failure: bool,
    ) -> AuthorizationState {
        if check_enforcement_failure {
            match self.store.enforcement_failure() {
                Ok(Some(_)) => {
                    return AuthorizationState::Denied(AuthorizationDenial::EnforcementUnhealthy)
                }
                Ok(None) => {}
                Err(_) => return AuthorizationState::Denied(AuthorizationDenial::AuditUnhealthy),
            }
        }
        let active_policy = match self.store.active_policy() {
            Ok(policy) => policy,
            Err(_) => return AuthorizationState::Denied(AuthorizationDenial::AuditUnhealthy),
        };
        let audit_healthy = self.store.audit_chain_healthy().unwrap_or(false);
        let last_trusted_at = match self.store.last_trusted_at() {
            Ok(timestamp) => timestamp,
            Err(_) => {
                return AuthorizationState::Denied(AuthorizationDenial::TrustedTimeUnavailable)
            }
        };

        let authorization =
            evaluate_authorization(active_policy.as_ref(), now, last_trusted_at, audit_healthy);
        if authorization == AuthorizationState::Allowed
            && self.store.record_trusted_time(now).is_err()
        {
            return AuthorizationState::Denied(AuthorizationDenial::TrustedTimeUnavailable);
        }
        authorization
    }

    /// 处理 Credential Provider 可调用的有限请求集。
    pub fn handle_request(
        &self,
        request: ServiceRequest,
        user: &str,
        now: DateTime<Utc>,
    ) -> ServiceResponse {
        match request {
            ServiceRequest::GetAuthorization => ServiceResponse::Authorization(
                authorization_response(self.authorization_state(now)),
            ),
            ServiceRequest::ImportFromPolicyMedia => {
                let outcome = self.import_from_policy_media(user, now);
                ServiceResponse::Import(ImportResponse {
                    accepted: outcome.accepted,
                    reason: outcome.reason,
                })
            }
        }
    }

    /// 处理 Named Pipe 传入的 JSON 载荷。
    ///
    /// 长度前缀由 `anchor-system::pipe` 负责，业务层仅编解码请求和响应 JSON，
    /// 以避免在 Credential Provider 与服务之间重复封帧。
    pub fn handle_json_request(
        &self,
        request: &[u8],
        user: &str,
        now: DateTime<Utc>,
    ) -> Result<Vec<u8>, serde_json::Error> {
        let request = serde_json::from_slice(request)?;
        serde_json::to_vec(&self.handle_request(request, user, now))
    }

    /// 处理用户会话状态端点传入的 JSON 授权查询。
    ///
    /// 此端点面向已登录用户，固定只读；策略导入仍只能通过受 `SYSTEM` 限制的控制端点
    /// 发起，避免会话协作程序获得导入能力。
    pub fn handle_session_status_request(
        &self,
        request: &[u8],
        now: DateTime<Utc>,
    ) -> Result<Vec<u8>, PlatformError> {
        let request = serde_json::from_slice(request)
            .map_err(|error| PlatformError::Failed(format!("会话状态 JSON 协议错误: {error}")))?;
        let ServiceRequest::GetAuthorization = request else {
            return Err(PlatformError::Failed("会话状态端点只接受授权查询".into()));
        };
        serde_json::to_vec(&ServiceResponse::Authorization(authorization_response(
            self.authorization_state(now),
        )))
        .map_err(|error| PlatformError::Failed(format!("会话状态 JSON 编码错误: {error}")))
    }

    /// 在固定控制端点处理一条 Credential Provider 请求。
    ///
    /// Credential Provider 由 LogonUI 承载，审计主体使用受控进程名称而不是客户端
    /// 可伪造的输入。Windows Service 的事件循环应重复调用本方法。
    pub fn serve_control_pipe_once(
        &self,
        now: DateTime<Utc>,
    ) -> Result<PipeServeOutcome, PlatformError> {
        serve_once(DEFAULT_CONTROL_PIPE, |request| {
            self.handle_json_request(request, "LogonUI", now)
                .map_err(|error| {
                    PlatformError::Failed(format!("Named Pipe JSON 协议错误: {error}"))
                })
        })
    }

    /// 在会话状态端点处理一条仅查询授权的请求。
    pub fn serve_session_status_pipe_once(
        &self,
        now: DateTime<Utc>,
    ) -> Result<PipeServeOutcome, PlatformError> {
        serve_session_status_once(|request| self.handle_session_status_request(request, now))
    }

    /// 枚举本地就绪卷根目录后导入固定策略文件。
    pub fn import_from_policy_media(&self, user: &str, now: DateTime<Utc>) -> ImportOutcome {
        let roots = policy_media_roots();
        let roots: Vec<&Path> = roots.iter().map(Path::new).collect();
        self.import_from_roots(&roots, user, now)
    }

    /// 从调用方提供的根目录导入策略。此接口同时用于 Windows 枚举和无设备依赖测试。
    pub fn import_from_roots(
        &self,
        roots: &[&Path],
        user: &str,
        now: DateTime<Utc>,
    ) -> ImportOutcome {
        let mut found_policy = false;
        let mut last_rejection = None;

        for root in roots {
            let policy_path = root.join(POLICY_FILE_NAME);
            if !policy_path.is_file() {
                continue;
            }
            found_policy = true;

            let package = match fs::read(&policy_path)
                .ok()
                .and_then(|content| serde_json::from_slice(&content).ok())
            {
                Some(package) => package,
                None => {
                    let reason = "policy_parse_failed";
                    if self.append_import_rejection(user, now, reason).is_err() {
                        return ImportOutcome::denied("audit_write_failed");
                    }
                    last_rejection = Some(reason);
                    continue;
                }
            };
            let current_version = match self.store.active_policy() {
                Ok(package) => package.map(|package| package.payload.version),
                Err(_) => return ImportOutcome::denied("store_unavailable"),
            };
            let Some(verifying_key) = self.verifying_key() else {
                if self
                    .append_import_rejection(user, now, "verifying_key_unavailable")
                    .is_err()
                {
                    return ImportOutcome::denied("audit_write_failed");
                }
                return ImportOutcome::denied("verifying_key_unavailable");
            };
            let decision = decide_import(&package, &verifying_key, current_version, now);
            if self
                .store
                .import_policy(&package, decision, &self.device_id, user, now)
                .is_err()
            {
                return ImportOutcome::denied("store_unavailable");
            }
            if decision == ImportDecision::Accept {
                return ImportOutcome::accepted();
            }
            last_rejection = Some(import_rejection_reason(decision));
        }

        if !found_policy {
            if self
                .append_import_rejection(user, now, "policy_not_found")
                .is_err()
            {
                return ImportOutcome::denied("audit_write_failed");
            }
            return ImportOutcome::denied("policy_not_found");
        }
        ImportOutcome::denied(last_rejection.unwrap_or("policy_not_accepted"))
    }

    fn append_import_rejection(
        &self,
        user: &str,
        now: DateTime<Utc>,
        reason: &str,
    ) -> Result<(), anchor_store::StoreError> {
        let mut event = AuditPayload::new(
            &self.device_id,
            user,
            EventType::PolicyImportRejected,
            "import_policy",
            reason,
        );
        event.timestamp = now;
        self.store.append_audit(event)
    }
}

impl ImportOutcome {
    fn accepted() -> Self {
        Self {
            accepted: true,
            reason: "accepted".into(),
        }
    }

    fn denied(reason: impl Into<String>) -> Self {
        Self {
            accepted: false,
            reason: reason.into(),
        }
    }
}

fn import_rejection_reason(decision: ImportDecision) -> &'static str {
    match decision {
        ImportDecision::Accept => "accepted",
        ImportDecision::InvalidSignature => "invalid_signature",
        ImportDecision::VersionNotNewer => "version_not_newer",
        ImportDecision::Expired => "expired",
    }
}

fn authorization_response(state: AuthorizationState) -> AuthorizationResponse {
    match state {
        AuthorizationState::Allowed => AuthorizationResponse::Allowed,
        AuthorizationState::Denied(reason) => AuthorizationResponse::Denied {
            reason: match reason {
                AuthorizationDenial::NoActivePolicy => "no_active_policy",
                AuthorizationDenial::Expired => "expired",
                AuthorizationDenial::TrustedTimeUnavailable => "trusted_time_unavailable",
                AuthorizationDenial::ClockRollback => "clock_rollback",
                AuthorizationDenial::AuditUnhealthy => "audit_unhealthy",
                AuthorizationDenial::EnforcementUnhealthy => "enforcement_unhealthy",
            }
            .into(),
        },
    }
}
