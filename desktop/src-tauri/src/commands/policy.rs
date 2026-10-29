use std::fs;

use anchor_ipc::{
    AuditFilter, AuditPage, DesktopProtocolError, DesktopRequest, DesktopResponse, DesktopStatus,
    ImportResponse, PolicyInspection, MAX_POLICY_PACKAGE_BYTES,
};
use serde::Serialize;
use tauri::State;

use crate::app::state::{NativeState, PendingPolicy};
use crate::ipc;

#[derive(Debug, Serialize)]
pub struct CommandError {
    pub code: String,
    pub message: String,
}

impl CommandError {
    pub fn protocol(error: impl std::fmt::Display) -> Self {
        Self {
            code: "protocol_error".into(),
            message: error.to_string(),
        }
    }

    pub fn service(message: impl Into<String>) -> Self {
        Self {
            code: "service_unavailable".into(),
            message: message.into(),
        }
    }

    pub fn unexpected(message: impl Into<String>) -> Self {
        Self {
            code: "unexpected_response".into(),
            message: message.into(),
        }
    }
}

impl From<DesktopProtocolError> for CommandError {
    fn from(error: DesktopProtocolError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct InspectResult {
    pub pending_id: String,
    pub inspection: PolicyInspection,
}

#[tauri::command]
pub fn get_policy_status() -> Result<DesktopStatus, CommandError> {
    match ipc::request(&DesktopRequest::GetStatus)? {
        DesktopResponse::Status(status) => Ok(status),
        DesktopResponse::Error(error) => Err(error.into()),
        _ => Err(CommandError::unexpected("expected status response")),
    }
}

#[tauri::command]
pub fn inspect_policy_file(
    path: String,
    state: State<'_, NativeState>,
) -> Result<InspectResult, CommandError> {
    let metadata = fs::metadata(&path)
        .map_err(|error| CommandError::service(format!("无法读取策略文件: {error}")))?;
    if metadata.len() > MAX_POLICY_PACKAGE_BYTES as u64 {
        return Err(CommandError {
            code: "policy_too_large".into(),
            message: "策略文件超过 256 KiB".into(),
        });
    }
    let bytes = fs::read(&path)
        .map_err(|error| CommandError::service(format!("无法读取策略文件: {error}")))?;
    let package_json =
        String::from_utf8(bytes).map_err(|_| CommandError::protocol("策略文件不是 UTF-8 文本"))?;
    let inspection = match ipc::request(&DesktopRequest::InspectPolicy {
        package_json: package_json.clone(),
    })? {
        DesktopResponse::Inspection(inspection) => inspection,
        DesktopResponse::Error(error) => return Err(error.into()),
        _ => return Err(CommandError::unexpected("expected inspection response")),
    };
    let pending_id = uuid::Uuid::new_v4().to_string();
    *state.pending.lock().expect("pending policy mutex poisoned") = Some(PendingPolicy {
        id: pending_id.clone(),
        package_json,
    });
    Ok(InspectResult {
        pending_id,
        inspection,
    })
}

#[tauri::command]
pub fn import_pending_policy(
    pending_id: String,
    state: State<'_, NativeState>,
) -> Result<ImportResponse, CommandError> {
    let pending = {
        let mut guard = state.pending.lock().expect("pending policy mutex poisoned");
        let Some(pending) = guard.as_ref() else {
            return Err(CommandError {
                code: "pending_not_found".into(),
                message: "没有待导入的策略".into(),
            });
        };
        if pending.id != pending_id {
            return Err(CommandError {
                code: "pending_not_found".into(),
                message: "待导入策略已失效".into(),
            });
        }
        guard.take().expect("pending policy exists")
    };
    match ipc::request(&DesktopRequest::ImportPolicy {
        package_json: pending.package_json,
    })? {
        DesktopResponse::Import(response) => Ok(response),
        DesktopResponse::Error(error) => Err(error.into()),
        _ => Err(CommandError::unexpected("expected import response")),
    }
}

#[tauri::command]
pub fn discard_pending_policy(pending_id: String, state: State<'_, NativeState>) {
    let mut guard = state.pending.lock().expect("pending policy mutex poisoned");
    if guard
        .as_ref()
        .is_some_and(|pending| pending.id == pending_id)
    {
        *guard = None;
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct AuditQuery {
    pub filter: AuditFilter,
    pub after_sequence: Option<i64>,
    pub limit: u32,
}

#[tauri::command]
pub fn get_audit_page(query: AuditQuery) -> Result<AuditPage, CommandError> {
    let request = DesktopRequest::QueryAudit {
        filter: query.filter,
        after_sequence: query.after_sequence,
        limit: query.limit.min(200),
    };
    match ipc::request(&request)? {
        DesktopResponse::Audit(page) => Ok(page),
        DesktopResponse::Error(error) => Err(CommandError::from(error)),
        _ => Err(CommandError::unexpected("unexpected desktop response")),
    }
}
