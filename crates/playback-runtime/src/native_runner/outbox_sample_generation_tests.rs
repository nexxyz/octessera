use super::*;

fn full_config(generation: u64) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetAudioConfig {
        revision: generation,
        request_id: None,
        generation,
        config: serde_json::json!({}),
    }
}

fn instrument_slot(kind: &str) -> RuntimeAudioCommand {
    RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: serde_json::json!({
            "type": kind,
            "sample": {}
        }),
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

fn generations(commands: &[RuntimeAudioCommand]) -> (u64, u64) {
    let instrument = commands.iter().find_map(|command| match command {
        RuntimeAudioCommand::SetInstrumentSlot { generation, .. } => Some(*generation),
        _ => None,
    });
    let sample = commands.iter().find_map(|command| match command {
        RuntimeAudioCommand::SetSampleBankParam { generation, .. } => Some(*generation),
        _ => None,
    });
    (
        instrument.expect("instrument replacement"),
        sample.expect("sample scalar"),
    )
}

#[test]
fn sample_owner_follows_sampler_replacements_only() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    let _ = outbox.drain_audio_commands();

    for (kind, expected_instrument, expected_sample) in [
        ("sampler", 21, 21),
        ("synth", 22, 21),
        ("midi", 23, 21),
        ("none", 24, 21),
        ("sampler", 25, 25),
    ] {
        outbox.push_audio_command(instrument_slot(kind));
        outbox.push_audio_command(sample_param(0));
        let commands = outbox.drain_audio_commands();
        assert_eq!(
            generations(&commands),
            (expected_instrument, expected_sample)
        );
    }
}

#[test]
fn non_sampler_replacement_preserves_queued_sample_scalar() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    let _ = outbox.drain_audio_commands();
    outbox.push_audio_command(instrument_slot("sampler"));
    let _ = outbox.drain_audio_commands();

    outbox.push_audio_command(sample_param(0));
    outbox.push_audio_command(instrument_slot("synth"));
    let commands = outbox.drain_audio_commands();
    assert_eq!(generations(&commands), (22, 21));
    assert!(matches!(
        commands[0],
        RuntimeAudioCommand::SetSampleBankParam { generation: 21, .. }
    ));
}

#[test]
fn sampler_replacement_removes_queued_sample_scalar() {
    let mut outbox = NativeRunnerOutbox::default();
    outbox.push_audio_command(full_config(20));
    let _ = outbox.drain_audio_commands();
    outbox.push_audio_command(instrument_slot("sampler"));
    let _ = outbox.drain_audio_commands();

    outbox.push_audio_command(sample_param(0));
    outbox.push_audio_command(instrument_slot("sampler"));
    let commands = outbox.drain_audio_commands();
    assert!(matches!(
        commands.as_slice(),
        [RuntimeAudioCommand::SetInstrumentSlot { generation: 22, .. }]
    ));
}

#[test]
fn repeated_full_barriers_rebaseline_sample_owner() {
    let mut outbox = NativeRunnerOutbox::default();
    for generation in [20, 30, 40] {
        outbox.push_audio_command(full_config(generation));
        outbox.push_audio_command(sample_param(0));
        let commands = outbox.drain_audio_commands();
        assert!(matches!(
            commands.as_slice(),
            [
                RuntimeAudioCommand::SetAudioConfig { generation: full, .. },
                RuntimeAudioCommand::SetSampleBankParam { generation: sample, .. }
            ] if *full == generation && *sample == generation
        ));
    }
}
