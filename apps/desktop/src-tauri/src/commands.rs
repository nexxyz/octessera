use crate::runtime_worker::{request_worker_dispatch, DISPATCH_REPLY_TIMEOUT};
use crate::types::{encode_runtime_responses, RuntimeMessagesPayload};
use playback_runtime::HostMessage;
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

/// Runs off the main thread, so a slow or stuck worker never freezes the window.
#[tauri::command(async)]
pub(crate) fn runtime_dispatch(
    message: Value,
    state: tauri::State<crate::AppState>,
) -> Result<Vec<Value>, String> {
    encode_runtime_responses(request_worker_dispatch(
        &state.worker_tx,
        ui_host_message(message)?,
        DISPATCH_REPLY_TIMEOUT,
    )?)
}

/// The webview may send only what the simulator UI sends: device input and the snapshot pulse.
fn ui_host_message(message: Value) -> Result<HostMessage, String> {
    let host_message = serde_json::from_value::<HostMessage>(message)
        .map_err(|e| format!("invalid runtime host message: {e}"))?;
    match host_message {
        HostMessage::DeviceInput { .. } | HostMessage::TransportPulseStep { .. } => {
            Ok(host_message)
        }
        _ => Err("runtime host message not accepted from the UI".into()),
    }
}

#[cfg(test)]
mod tests;
