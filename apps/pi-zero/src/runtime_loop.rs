use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    CoreRunner, HostAdapter, HostMessage, NativeRunner, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimePlatformEffect, SyncSource,
};
use serde_json::Value;
use std::collections::VecDeque;

pub fn dispatch_runtime_message(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
    host_message: HostMessage,
) -> Result<(), String> {
    let responses = dispatch_and_ingest(playback, runner, adapter, host_message)?;
    ingest_responses(playback, adapter, responses)
}

pub fn handle_deferred_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) -> Result<(), String> {
    let follow_ups = adapter.flush_due_default_save()?;
    for follow_up in follow_ups {
        dispatch_runtime_message(playback, runner, adapter, follow_up)?;
    }
    Ok(())
}

pub fn initialize_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
) -> Result<(), String> {
    for effect in [
        RuntimePlatformEffect::StoreLoadDefault,
        RuntimePlatformEffect::MidiListOutputsRequest,
        RuntimePlatformEffect::MidiListInputsRequest,
    ] {
        for follow_up in adapter.handle_platform_effect(&effect)? {
            dispatch_runtime_message(playback, runner, adapter, follow_up)?;
        }
    }
    Ok(())
}

pub fn latest_snapshot(playback: &PlaybackRuntime) -> Option<&Value> {
    playback.last_snapshot()
}

pub fn sync_playback_config_from_snapshot(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    snapshot: &Value,
) {
    let Some(config) = playback_config_from_snapshot(snapshot) else {
        return;
    };
    if playback.config() == &config {
        return;
    }
    playback.set_config(config);
    runner.apply_runtime_config(playback.config());
}

fn playback_config_from_snapshot(snapshot: &Value) -> Option<RuntimeConfig> {
    let midi = snapshot.get("settings")?.get("midi")?;
    let transport = snapshot.get("transport").unwrap_or(&Value::Null);
    let sync_source = match midi.get("syncMode").and_then(Value::as_str) {
        Some("external") => SyncSource::External,
        _ => SyncSource::Internal,
    };
    let midi_enabled = midi
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let midi_out_selected = midi.get("outId").is_some_and(|value| !value.is_null());
    Some(RuntimeConfig {
        bpm: transport
            .get("bpm")
            .and_then(Value::as_f64)
            .unwrap_or(120.0),
        sync_source,
        midi_clock_out_enabled: midi
            .get("clockOutEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        midi_out_enabled: midi_enabled && midi_out_selected,
    })
}

fn dispatch_and_ingest(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiPlaybackHostAdapter<'_>,
    host_message: HostMessage,
) -> Result<Vec<RunnerMessage>, String> {
    let mut captured = Vec::new();
    let mut queue = VecDeque::from([host_message]);
    while let Some(message) = queue.pop_front() {
        let responses = runner.send(message)?;
        captured.extend(responses.iter().cloned());
        for follow_up in playback.ingest_runner_messages(responses, adapter)? {
            queue.push_back(follow_up);
        }
    }
    Ok(captured)
}

fn ingest_responses(
    _playback: &mut PlaybackRuntime,
    _adapter: &mut PiPlaybackHostAdapter<'_>,
    _responses: Vec<RunnerMessage>,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn playback_config_from_snapshot_tracks_midi_runtime_settings() {
        let snapshot = json!({
            "transport": { "bpm": 93.5 },
            "settings": {
                "midi": {
                    "enabled": true,
                    "outId": "0",
                    "syncMode": "external",
                    "clockOutEnabled": true
                }
            }
        });

        let config = playback_config_from_snapshot(&snapshot).unwrap();

        assert_eq!(config.bpm, 93.5);
        assert_eq!(config.sync_source, SyncSource::External);
        assert!(config.midi_clock_out_enabled);
        assert!(config.midi_out_enabled);
    }
}
