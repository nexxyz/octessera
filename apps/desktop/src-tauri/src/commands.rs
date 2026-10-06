use crate::runtime_worker::request_worker_dispatch;
use crate::types::{encode_runtime_responses, RuntimeMessagesPayload};
use serde_json::Value;

#[tauri::command]
pub(crate) fn runtime_drain_messages(
    state: tauri::State<crate::AppState>,
) -> Result<Vec<RuntimeMessagesPayload>, String> {
    let mut guard = state
        .runtime_outbox
        .lock()
        .map_err(|_| "runtime outbox mutex poisoned".to_string())?;
    Ok(std::mem::take(&mut *guard))
}

#[tauri::command]
pub(crate) fn runtime_dispatch(
    message: Value,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    let host_message = serde_json::from_value::<playback_runtime::HostMessage>(message)
        .map_err(|e| format!("invalid runtime host message: {e}"))?;
    encode_runtime_responses(request_worker_dispatch(&state, host_message)?)
}
