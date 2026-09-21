use super::*;
use platform_core::{BUS_COUNT, GLOBAL_FX_SLOT_COUNT};
use realtime_engine::synth::{FxParamId, BUS_SLOTS_PER_BUS};
use std::collections::BTreeMap;

fn full_config(generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetAudioConfig {
        revision: generation,
        request_id: None,
        generation,
        config: serde_json::json!({}),
    }
}

fn instrument_mixer(generation: u64, volume_pct: f32) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetInstrumentMixer {
        instrument_slot: 0,
        generation,
        volume_pct: Some(volume_pct),
        pan_pos: None,
    }
}

fn synth_param(generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetSynthParam {
        instrument_slot: 0,
        generation,
        path: "filter.cutoff".into(),
        value: 440.0,
    }
}

fn sample_param(generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetSampleBankParam {
        instrument_slot: 0,
        generation,
        path: "sample.tune".into(),
        value: 2.0,
    }
}

fn bus_mixer(bus_index: usize, generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetFxBusMixer {
        bus_index,
        generation,
        pan_pos: Some(4),
        volume_pct: None,
    }
}

fn bus_param(bus_index: usize, slot_index: usize, generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetFxBusParam {
        bus_index,
        slot_index,
        generation,
        param: FxParamId::MixPct,
        value: 0.5,
    }
}

fn bus_slot(bus_index: usize, slot_index: usize, generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetFxBusSlot {
        bus_index,
        slot_index,
        generation,
        fx_type: "eq".into(),
        params: BTreeMap::new(),
    }
}

fn global_param(slot_index: usize, generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetGlobalFxParam {
        slot_index,
        generation,
        param: FxParamId::MixPct,
        value: 0.5,
    }
}

fn global_slot(slot_index: usize, generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetGlobalFxSlot {
        slot_index,
        generation,
        fx_type: "eq".into(),
        params: BTreeMap::new(),
    }
}

fn drain_one(outbox: &mut NativeRunnerOutbox) -> RuntimeAudioCommand {
    let mut commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), 1, "{commands:?}");
    commands.remove(0)
}

#[test]
fn rodio_generation_fixture_rebases_scalar_and_replacement_sequences() {
    let mut outbox = NativeRunnerOutbox::default();
    let mut sequence = Vec::new();

    outbox.push_audio_command(full_config(20));
    sequence.push(drain_one(&mut outbox));
    outbox.push_audio_command(instrument_mixer(0, 40.0));
    sequence.push(drain_one(&mut outbox));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({ "type": "synth" }),
    });
    sequence.push(drain_one(&mut outbox));
    outbox.push_audio_command(instrument_mixer(0, 50.0));
    sequence.push(drain_one(&mut outbox));
    outbox.push_audio_command(full_config(30));
    sequence.push(drain_one(&mut outbox));
    outbox.push_audio_command(instrument_mixer(0, 60.0));
    sequence.push(drain_one(&mut outbox));

    assert!(matches!(
        sequence[0],
        RuntimeAudioCommand::SetAudioConfig { generation: 20, .. }
    ));
    assert!(matches!(
        sequence[1],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 20, .. }
    ));
    assert!(matches!(
        sequence[2],
        RuntimeAudioCommand::SetInstrumentSlot { generation: 21, .. }
    ));
    assert!(matches!(
        sequence[3],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 21, .. }
    ));
    assert!(matches!(
        sequence[4],
        RuntimeAudioCommand::SetAudioConfig { generation: 30, .. }
    ));
    assert!(matches!(
        sequence[5],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 30, .. }
    ));
}

#[test]
fn full_barrier_baselines_each_instrument_scalar_and_replacement_owner() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    outbox.push_audio_command(instrument_mixer(0, 40.0));
    outbox.push_audio_command(synth_param(0));
    outbox.push_audio_command(sample_param(0));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetAudioConfig { generation: 20, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 20, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::SetSynthParam { generation: 20, .. }
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::SetSampleBankParam { generation: 20, .. }
    ));

    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({ "type": "synth" }),
    });
    outbox.push_audio_command(instrument_mixer(0, 50.0));
    outbox.push_audio_command(synth_param(0));
    outbox.push_audio_command(sample_param(0));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetInstrumentSlot { generation: 21, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 21, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::SetSynthParam { generation: 21, .. }
    ));
    assert!(matches!(
        commands[3],
        RuntimeAudioCommand::SetSampleBankParam { generation: 20, .. }
    ));

    outbox.push_audio_command(full_config(30));
    outbox.push_audio_command(instrument_mixer(0, 60.0));
    outbox.push_audio_command(synth_param(0));
    outbox.push_audio_command(sample_param(0));
    let commands = outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetInstrumentMixer { generation, .. }
        | RuntimeAudioCommand::SetSynthParam { generation, .. }
        | RuntimeAudioCommand::SetSampleBankParam { generation, .. } => *generation == 30,
        _ => false,
    }));
}

#[test]
fn full_barrier_baselines_bus_mixer_and_every_bus_slot_shape() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    for bus_index in 0..BUS_COUNT {
        outbox.push_audio_command(bus_mixer(bus_index, 0));
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            outbox.push_audio_command(bus_param(bus_index, slot_index, 0));
        }
    }
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), 1 + BUS_COUNT * (1 + BUS_SLOTS_PER_BUS));
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetAudioConfig { generation: 20, .. }
    ));
    for bus_index in 0..BUS_COUNT {
        assert!(commands.iter().any(|command| matches!(
            command,
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index: actual,
                generation: 20,
                ..
            } if *actual == bus_index
        )));
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            assert!(commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusParam {
                    bus_index: actual_bus,
                    slot_index: actual_slot,
                    generation: 20,
                    ..
                } if *actual_bus == bus_index && *actual_slot == slot_index
            )));
        }
    }

    for bus_index in 0..BUS_COUNT {
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            outbox.push_audio_command(bus_slot(bus_index, slot_index, 0));
            outbox.push_audio_command(bus_param(bus_index, slot_index, 0));
        }
    }
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), BUS_COUNT * BUS_SLOTS_PER_BUS * 2);
    for bus_index in 0..BUS_COUNT {
        for slot_index in 0..BUS_SLOTS_PER_BUS {
            assert!(commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusSlot {
                    bus_index: actual_bus,
                    slot_index: actual_slot,
                    generation: 21,
                    ..
                } if *actual_bus == bus_index && *actual_slot == slot_index
            )));
            assert!(commands.iter().any(|command| matches!(
                command,
                RuntimeAudioCommand::SetFxBusParam {
                    bus_index: actual_bus,
                    slot_index: actual_slot,
                    generation: 21,
                    ..
                } if *actual_bus == bus_index && *actual_slot == slot_index
            )));
        }
    }

    outbox.push_audio_command(full_config(30));
    outbox.push_audio_command(bus_mixer(0, 0));
    outbox.push_audio_command(bus_param(0, 0, 0));
    let commands = outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetFxBusMixer { generation, .. }
        | RuntimeAudioCommand::SetFxBusParam { generation, .. } => *generation == 30,
        _ => false,
    }));
}

#[test]
fn full_barrier_baselines_every_global_slot_and_replacement_owner() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    for slot_index in 0..GLOBAL_FX_SLOT_COUNT {
        outbox.push_audio_command(global_param(slot_index, 0));
    }
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), 1 + GLOBAL_FX_SLOT_COUNT);
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetGlobalFxParam { generation, .. } => *generation == 20,
        _ => false,
    }));

    for slot_index in 0..GLOBAL_FX_SLOT_COUNT {
        outbox.push_audio_command(global_slot(slot_index, 0));
        outbox.push_audio_command(global_param(slot_index, 0));
    }
    let commands = outbox.drain_audio_commands();
    assert_eq!(commands.len(), GLOBAL_FX_SLOT_COUNT * 2);
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetGlobalFxSlot { generation, .. }
        | RuntimeAudioCommand::SetGlobalFxParam { generation, .. } => *generation == 21,
        _ => false,
    }));

    outbox.push_audio_command(full_config(30));
    outbox.push_audio_command(global_param(0, 0));
    let commands = outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetGlobalFxParam { generation, .. } => *generation == 30,
        _ => false,
    }));
}

#[test]
fn explicit_generations_never_regress_an_owner() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    outbox.push_audio_command(instrument_mixer(0, 40.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 19,
        config: serde_json::json!({}),
    });
    outbox.push_audio_command(instrument_mixer(0, 50.0));
    let commands = outbox.drain_audio_commands();

    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetAudioConfig { generation: 20, .. }
    ));
    assert!(matches!(
        commands[1],
        RuntimeAudioCommand::SetInstrumentSlot { generation: 21, .. }
    ));
    assert!(matches!(
        commands[2],
        RuntimeAudioCommand::SetInstrumentMixer { generation: 21, .. }
    ));
}

#[test]
fn explicit_generations_never_regress_fx_bus_or_global_owners() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    outbox.push_audio_command(bus_mixer(0, 19));
    outbox.push_audio_command(bus_param(0, 0, 19));
    outbox.push_audio_command(bus_slot(0, 1, 19));
    outbox.push_audio_command(global_param(0, 19));
    outbox.push_audio_command(global_slot(1, 19));
    let commands = outbox.drain_audio_commands();

    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetFxBusMixer { generation: 20, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetFxBusParam { generation: 20, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetFxBusSlot { generation: 21, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetGlobalFxParam { generation: 20, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SetGlobalFxSlot { generation: 21, .. }
    )));

    outbox.push_audio_command(bus_param(0, 1, 19));
    outbox.push_audio_command(global_param(1, 19));
    let commands = outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetFxBusParam { generation, .. }
        | RuntimeAudioCommand::SetGlobalFxParam { generation, .. } => *generation == 21,
        _ => false,
    }));
}

#[test]
fn full_generation_and_replacement_increment_saturate_at_u64_max() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(u64::MAX));
    outbox.push_audio_command(instrument_mixer(0, 40.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({}),
    });
    outbox.push_audio_command(instrument_mixer(0, 50.0));
    let commands = outbox.drain_audio_commands();

    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetInstrumentMixer { generation, .. }
        | RuntimeAudioCommand::SetInstrumentSlot { generation, .. } => {
            *generation == u64::MAX
        }
        _ => false,
    }));

    outbox.push_audio_command(full_config(u64::MAX - 1));
    outbox.push_audio_command(instrument_mixer(0, 60.0));
    let commands = outbox.drain_audio_commands();
    assert!(commands.iter().all(|command| match command {
        RuntimeAudioCommand::SetAudioConfig { generation, .. }
        | RuntimeAudioCommand::SetInstrumentMixer { generation, .. } => *generation == u64::MAX,
        _ => false,
    }));
}

#[test]
fn momentary_epochs_and_preview_are_independent_of_generation_rebases() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxStart {
        id: "hold".into(),
        epoch: 0,
        fx_type: "stutter".into(),
        params: BTreeMap::new(),
        target: crate::protocol::RuntimeMomentaryFxTarget::Global,
    });
    outbox.push_audio_command(full_config(20));
    outbox.push_audio_command(RuntimeAudioCommand::MomentaryFxUpdate {
        id: "hold".into(),
        epoch: 0,
        params: BTreeMap::new(),
    });
    outbox.push_audio_command(RuntimeAudioCommand::SamplePreview {
        instrument_slot: 0,
        sample_slot: 0,
        path: "preview.wav".into(),
        velocity: 100,
    });
    let commands = outbox.drain_audio_commands();

    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::MomentaryFxStart { epoch: 1, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::MomentaryFxUpdate { epoch: 1, .. }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RuntimeAudioCommand::SamplePreview { path, .. } if path == "preview.wav"
    )));
}
