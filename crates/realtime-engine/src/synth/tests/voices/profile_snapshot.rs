use super::*;

#[test]
fn profile_snapshot_reports_active_counts_and_steals() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".into(),
                    pan_pos: 0,
                    volume: 100.0,
                }),
            },
            InstrumentSlotConfig {
                kind: "sampler".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: "direct".into(),
                    pan_pos: 1,
                    volume: 100.0,
                }),
            },
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    for note in 0..(MAX_SYNTH_VOICES_PER_SLOT + 1) {
        engine.note_on(0, (60 + note) as u8, 100, 2_000);
    }

    engine.set_sample_banks(vec![sample_bank(vec![1.0; 16_384]); INSTRUMENT_SLOT_COUNT]);
    engine.note_on(1, 36, 100, 1_000);
    engine.preview_sample(
        1,
        SampleBuffer {
            samples: vec![0.25; 256].into_boxed_slice().into(),
            channels: 1,
            sample_rate: 48_000,
        },
        100,
    );
    engine.momentary_fx_start(
        "fx-1".into(),
        "filter_sweep".into(),
        BTreeMap::from([(String::from("cutoffPct"), json!(50.0))]),
        MomentaryFxTarget::Global,
    );

    let snapshot = engine.profile_snapshot();

    assert_eq!(snapshot.active_synth_voices, MAX_SYNTH_VOICES_PER_SLOT);
    assert_eq!(snapshot.active_sample_voices, 1);
    assert_eq!(snapshot.active_preview_sample_voices, 1);
    assert_eq!(snapshot.active_momentary_fx, 1);
    assert_eq!(snapshot.cumulative_voice_steals, 1);
}
