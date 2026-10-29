use std::time::Duration;

use crate::commands::policy::CommandError;

pub fn request(
    request: &anchor_ipc::DesktopRequest,
) -> Result<anchor_ipc::DesktopResponse, CommandError> {
    let body = serde_json::to_vec(request).map_err(CommandError::protocol)?;
    let response = anchor_system::pipe::transact_desktop(&body, Duration::from_secs(2))
        .map_err(|error| CommandError::service(error.to_string()))?;
    serde_json::from_slice(&response).map_err(CommandError::protocol)
}
