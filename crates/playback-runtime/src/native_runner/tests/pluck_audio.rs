use super::*;

#[test]
fn pluck_outbox_scalar_serde_and_replacement_generation() {
    let param = |value| RuntimeAudioCommand::SetPluckParam {
        instrument_slot: 0,
        generation: 0,
        path: "pluck.pickPositionPct".into(),
        value,
    };
    let value = serde_json::to_value(param(26.0)).unwrap();
    assert_eq!(value["type"], "set_pluck_param");
    assert_eq!(value["instrumentSlot"], 0);
    assert_eq!(
        serde_json::from_value::<RuntimeAudioCommand>(value).unwrap(),
        param(26.0)
    );
    let mut outbox = super::super::outbox::NativeRunnerOutbox::default();
    outbox.push_audio_command(param(26.0));
    outbox.push_audio_command(param(27.0));
    assert!(matches!(
        &outbox.drain_audio_commands()[..],
        [RuntimeAudioCommand::SetPluckParam {
            value: 27.0,
            generation: 0,
            ..
        }]
    ));
    outbox.push_audio_command(param(28.0));
    outbox.push_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
        instrument_slot: 0,
        generation: 0,
        config: json!({ "type": "pluck" }),
    });
    assert!(matches!(
        &outbox.drain_audio_commands()[..],
        [RuntimeAudioCommand::SetInstrumentSlot { generation: 1, .. }]
    ));
    outbox.push_audio_command(param(29.0));
    assert!(matches!(
        &outbox.drain_audio_commands()[..],
        [RuntimeAudioCommand::SetPluckParam { generation: 1, .. }]
    ));
}

#[test]
fn pluck_transposed_held_notes_keep_internal_slot_and_mixer_route() {
    use crate::native_runner::modulation_sampler::apply_sampler_assignments_for_instruments_routed;
    use std::collections::BTreeMap;

    let mut instrument = NativeInstrumentSlot::new(0);
    instrument.kind = "pluck".into();
    instrument.route = "fx_bus_2".into();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0] = instrument.clone();
    assert!(runner.play_transpose_target_eligible(0, "note_on"));
    assert!(runner.play_transpose_target_eligible(0, "note_off"));
    let intent = platform_core::CellTriggerIntent {
        x: 2,
        y: 3,
        degree: 0,
        kind: platform_core::CellTriggerKind::Activate,
    };
    let mut held = BTreeMap::new();
    let note_on = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOn {
            channel: 0,
            note: 60,
            velocity: 90,
            duration_ms: None,
        }],
        std::slice::from_ref(&intent),
        0,
        std::slice::from_ref(&instrument),
        None,
        7,
        Some(&mut held),
    );
    assert!(note_on.midi.is_empty());
    assert!(matches!(
        &note_on.audio[..],
        [MusicalEvent::NoteOn {
            channel: 0,
            note: 67,
            velocity: 90,
            ..
        }]
    ));
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["mixer"]["route"],
        "fx_bus_2"
    );
    let note_off = apply_sampler_assignments_for_instruments_routed(
        vec![MusicalEvent::NoteOff {
            channel: 0,
            note: 60,
        }],
        &[intent],
        0,
        &[instrument],
        None,
        0,
        Some(&mut held),
    );
    assert!(note_off.midi.is_empty());
    assert!(matches!(
        &note_off.audio[..],
        [MusicalEvent::NoteOff {
            channel: 0,
            note: 67
        }]
    ));
    assert!(held.is_empty());
}

#[test]
fn pluck_scalar_paths_and_ranges_match_dsp_typed_ids() {
    let fields = [
        ("decayMs", 100, 5000),
        ("brightnessPct", 0, 100),
        ("pickPositionPct", 5, 50),
        ("amp.gainPct", 0, 100),
        ("amp.velocitySensitivityPct", 0, 100),
        ("ampEnv.attackMs", 0, 5000),
        ("ampEnv.decayMs", 0, 5000),
        ("ampEnv.sustainPct", 0, 100),
        ("ampEnv.releaseMs", 0, 10000),
        ("filter.cutoffHz", 0, 255),
        ("filter.resonance", 0, 255),
        ("filter.envAmountPct", -100, 100),
        ("filter.keyTrackingPct", 0, 100),
        ("filterEnv.attackMs", 0, 5000),
        ("filterEnv.decayMs", 0, 5000),
        ("filterEnv.sustainPct", 0, 100),
        ("filterEnv.releaseMs", 0, 10000),
    ];
    assert_eq!(
        fields.len(),
        realtime_engine::synth::PluckParamId::ALL.len()
    );
    for (suffix, min, max) in fields {
        let path = format!("pluck.{suffix}");
        assert!(
            realtime_engine::synth::PluckParamId::from_path(&path).is_some(),
            "{path}"
        );
        assert_eq!(
            super::super::menu_apply_fast_instruments::pluck::numeric_field(suffix)
                .map(|(_, low, high)| (low, high)),
            Some((min, max))
        );
        assert!(matches!(
            super::super::modulation_audio::instrument_modulation_audio_command(
                0,
                &path,
                &json!(max)
            ),
            Some(RuntimeAudioCommand::SetPluckParam { .. })
        ));
    }
}
