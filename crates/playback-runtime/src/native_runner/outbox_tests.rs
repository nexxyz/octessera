use super::*;
use realtime_engine::synth::FxParamId;
use std::collections::BTreeMap;

fn synth(generation: u64, path: &str, value: f32) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetSynthParam {
        instrument_slot: 0,
        generation,
        path: path.into(),
        value,
    }
}

#[test]
fn coalesces_ten_thousand_updates_per_scalar_key() {
    let mut outbox = NativeRunnerOutbox::default();
    for value in 0..10_000 {
        outbox.push_audio_command(synth(1, "filter.cutoff", value as f32));
    }
    assert_eq!(outbox.audio_commands.len(), 1);
    assert_eq!(outbox.audio_commands[0], synth(1, "filter.cutoff", 9_999.0));
}

#[test]
fn distinct_scalar_keys_survive_coalescing() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(synth(1, "filter.cutoff", 100.0));
    outbox.push_audio_command(synth(1, "filter.resonance", 20.0));
    assert_eq!(outbox.audio_commands.len(), 2);
}

#[test]
fn owner_stamps_increment_only_on_replacement_boundaries() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(RuntimeAudioCommand::SetAudioConfig {
        revision: 7,
        request_id: None,
        generation: 0,
        config: serde_json::json!({}),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetDspConfig {
        generation: 0,
        config: realtime_engine::synth::DspRuntimeConfig::default(),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({}),
    });
    outbox.push_audio_command(synth(0, "filter.cutoff", 100.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({}),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        generation: 0,
        fx_type: "eq".into(),
        params: BTreeMap::new(),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusParam {
        bus_index: 0,
        slot_index: 0,
        generation: 0,
        param: FxParamId::MixPct,
        value: 0.5,
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStart {
        id: "x".into(),
        epoch: 0,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
        id: "x".into(),
        epoch: 0,
        params: BTreeMap::new(),
    });
    let commands = outbox.drain_audio_commands();
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetAudioConfig { generation: 7, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::SetDspConfig { generation: 7, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::SetInstrumentSlot { generation: 9, .. }
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::SetFxBusSlot { generation: 8, .. }
    ));
    assert!(matches!(
        commands[4],
        RuntimeAudioCommand::SetFxBusParam { generation: 8, .. }
    ));
    assert!(matches!(
        commands[5],
        RuntimeAudioCommand::MomentaryFxStart { epoch: 1, .. }
    ));
    assert!(matches!(
        commands[6],
        RuntimeAudioCommand::MomentaryFxUpdate { epoch: 1, .. }
    ));
}

#[test]
fn full_config_is_first_but_keeps_momentary_lifecycle_and_latest_preview() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(synth(1, "filter.cutoff", 100.0));
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStart {
        id: "x".into(),
        epoch: 1,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
        id: "x".into(),
        epoch: 1,
        params: BTreeMap::from([(String::from("rateHz"), serde_json::json!(2))]),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SamplePreview {
        instrument_slot: 0,
        sample_slot: 0,
        path: "old.wav".into(),
        velocity: 100,
    });
    outbox.push_audio_command(RuntimeAudioCommand::SamplePreview {
        instrument_slot: 0,
        sample_slot: 0,
        path: "new.wav".into(),
        velocity: 100,
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetAudioConfig {
        revision: 4,
        request_id: None,
        generation: 4,
        config: serde_json::json!({}),
    });
    let commands = outbox.drain_audio_commands();
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetAudioConfig { .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::MomentaryFxStart { .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::MomentaryFxUpdate { .. }
    ));
    assert!(
        matches!(commands[3], RuntimeAudioCommand::SamplePreview { ref path, .. } if path == "new.wav")
    );
}

#[test]
fn slot_replacement_removes_only_its_owner_and_later_scalar_survives() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusParam {
        bus_index: 0,
        slot_index: 0,
        generation: 1,
        param: FxParamId::MixPct,
        value: 0.2,
    });
    outbox.push_audio_command(synth(1, "filter.cutoff", 100.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusSlot {
        bus_index: 0,
        slot_index: 0,
        generation: 2,
        fx_type: "delay".into(),
        params: BTreeMap::new(),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusParam {
        bus_index: 0,
        slot_index: 0,
        generation: 2,
        param: FxParamId::MixPct,
        value: 0.4,
    });
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), 3);
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetSynthParam { .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::SetFxBusSlot { .. }
    ));
    assert!(
        matches!(commands[2], RuntimeAudioCommand::SetFxBusParam { value, .. } if value == 0.4)
    );
}

#[test]
fn momentary_updates_are_latest_only_and_epoch_isolated() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStart {
        id: "x".into(),
        epoch: 1,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    });
    for value in 0..10_000 {
        outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
            id: "x".into(),
            epoch: 1,
            params: BTreeMap::from([(String::from("rateHz"), serde_json::json!(value))]),
        });
    }
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStop {
        id: "x".into(),
        epoch: 1,
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStart {
        id: "x".into(),
        epoch: 2,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
        id: "x".into(),
        epoch: 1,
        params: BTreeMap::from([(String::from("rateHz"), serde_json::json!(99))]),
    });
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
        id: "x".into(),
        epoch: 2,
        params: BTreeMap::from([(String::from("rateHz"), serde_json::json!(2))]),
    });
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), 5);
    assert!(
        matches!(commands[1], RuntimeAudioCommand::MomentaryFxUpdate { ref params, .. } if params.get("rateHz") == Some(&serde_json::json!(9_999)))
    );
    assert!(matches!(
        commands[4],
        RuntimeAudioCommand::MomentaryFxUpdate { epoch: 2, .. }
    ));
}

#[test]
fn merges_instrument_and_bus_mixer_partials() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
        instrument_slot: 0,
        generation: 1,
        volume_pct: Some(55.0),
        pan_pos: None,
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
        instrument_slot: 0,
        generation: 1,
        volume_pct: None,
        pan_pos: Some(12),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusMixer {
        bus_index: 0,
        generation: 1,
        pan_pos: Some(12),
        volume_pct: None,
    });
    outbox.push_audio_command(RuntimeAudioCommand::SetFxBusMixer {
        bus_index: 0,
        generation: 1,
        pan_pos: None,
        volume_pct: Some(60.0),
    });
    assert_eq!(outbox.audio_commands.len(), 2);
    assert!(matches!(
        outbox.audio_commands[0],
        RuntimeAudioCommand::SetInstrumentMixer {
            volume_pct: Some(55.0),
            pan_pos: Some(12),
            ..
        }
    ));
    assert!(matches!(
        outbox.audio_commands[1],
        RuntimeAudioCommand::SetFxBusMixer {
            volume_pct: Some(60.0),
            pan_pos: Some(12),
            ..
        }
    ));
}
