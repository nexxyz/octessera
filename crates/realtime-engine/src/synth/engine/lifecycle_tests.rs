use super::*;

fn sample_buffer(value: f32, frames: usize) -> SampleBuffer {
    SampleBuffer {
        samples: vec![value; frames].into(),
        channels: 1,
        sample_rate: 48_000,
    }
}

#[test]
fn preview_slots_replace_oldest_and_retire_displaced_state() {
    let mut engine = SynthEngine::new(48_000);
    assert!(engine
        .preview_sample(0, sample_buffer(1.0, 64), 100)
        .preview_sample_voices
        .iter()
        .all(Option::is_none));
    assert!(engine
        .preview_sample(0, sample_buffer(2.0, 64), 100)
        .preview_sample_voices
        .iter()
        .all(Option::is_none));

    let retired = engine.preview_sample(0, sample_buffer(3.0, 64), 100);

    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 2);
    let retired_values: Vec<f32> = retired
        .preview_sample_voices
        .iter()
        .flatten()
        .map(|voice| voice.buffer.samples[0])
        .collect();
    assert_eq!(retired_values, vec![1.0]);
    let active_values: Vec<f32> = engine
        .preview_sample_voices
        .iter()
        .flatten()
        .map(|voice| voice.buffer.samples[0])
        .collect();
    assert_eq!(active_values, vec![3.0, 2.0]);
}

#[test]
fn completed_preview_moves_to_pending_render_retirement() {
    let mut engine = SynthEngine::new(48_000);
    let _ = engine.preview_sample(0, sample_buffer(1.0, 1), 100);

    let _ = engine.next_stereo_sample();

    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 0);
    assert!(!engine.pending_render_retired_is_empty());
    let retired = engine.take_pending_render_retired();
    assert_eq!(retired.preview_sample_voices.iter().flatten().count(), 1);
    assert!(engine.pending_render_retired_is_empty());
}

#[test]
fn invalid_preview_moves_its_buffer_to_retirement() {
    let mut engine = SynthEngine::new(48_000);

    let retired = engine.preview_sample(
        0,
        SampleBuffer {
            samples: vec![1.0].into(),
            channels: 0,
            sample_rate: 48_000,
        },
        100,
    );

    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 0);
    assert_eq!(retired.preview_sample_buffers.iter().flatten().count(), 1);
    assert!(engine.is_idle());
}

#[test]
fn all_notes_off_returns_preview_payloads_for_retirement() {
    let mut engine = SynthEngine::new(48_000);
    let _ = engine.preview_sample(0, sample_buffer(1.0, 64), 100);
    let _ = engine.preview_sample(0, sample_buffer(2.0, 64), 100);

    let retired = engine.all_notes_off();

    assert_eq!(engine.profile_snapshot().active_preview_sample_voices, 0);
    assert_eq!(retired.preview_sample_voices.iter().flatten().count(), 2);
    assert!(engine.is_idle());
}

#[test]
fn sample_bank_replacement_retires_lane_handle_without_callback_deallocation() {
    let mut engine = sample_engine();
    let _ = engine.set_sample_banks(vec![sample_bank(1.0)]);
    engine.note_on(0, 36, 100, 2_000);
    let old_samples = engine
        .sample_voice_pool
        .lane(0)
        .and_then(|voice| voice.buffer.as_ref())
        .expect("sample voice owns its buffer")
        .samples
        .clone();

    let replacement = sample_bank(2.0);
    let (retired, _, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.apply_prepared_sample_bank(0, replacement)
        });

    assert_eq!(deallocations, 0);
    assert_eq!(retired.sample_voices.len(), 1);
    let retired_samples = retired
        .sample_voices
        .get(0)
        .expect("retired sample voice")
        .buffer
        .as_ref()
        .expect("retired sample voice owns its buffer")
        .samples
        .clone();
    assert!(std::sync::Arc::ptr_eq(&old_samples, &retired_samples));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
    drop(retired);
}

#[test]
fn all_notes_off_retires_lane_held_sample_state() {
    let mut engine = sample_engine();
    let _ = engine.set_sample_banks(vec![sample_bank(1.0)]);
    engine.note_on(0, 36, 100, 2_000);

    let (retired, _, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            engine.all_notes_off()
        });

    assert_eq!(deallocations, 0);
    assert_eq!(retired.sample_voices.len(), 1);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
    assert!(engine
        .sample_voice_pool
        .lane(0)
        .expect("home partition lane")
        .buffer
        .is_none());
}

#[test]
fn sample_lane_reuse_moves_previous_handle_to_pending_retirement() {
    let mut engine = sample_engine();
    let _ = engine.set_sample_banks(vec![sample_bank(1.0)]);
    for _ in 0..9 {
        engine.note_on(0, 36, 100, 2_000);
    }

    assert_eq!(engine.profile_snapshot().active_sample_voices, 8);
    assert_eq!(engine.pending_render_retired.sample_voices.len(), 1);
}

#[test]
fn active_sample_partition_round_trip_preserves_voice_and_metadata() {
    let mut engine = sample_engine();
    engine.set_instrument_slot(
        1,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    let _ = engine.set_sample_banks(vec![sample_bank(1.0); INSTRUMENT_SLOT_COUNT]);
    engine.note_on(0, 36, 100, 2_000);
    engine.note_on(1, 36, 100, 2_000);
    let metadata: Vec<Vec<usize>> = (0..INSTRUMENT_SLOT_COUNT)
        .map(|slot| {
            engine
                .sample_voice_pool
                .slot_lanes(slot)
                .expect("home partition lanes")
                .to_vec()
        })
        .collect();
    let states: Vec<String> = [0, 1]
        .into_iter()
        .map(|lane| format!("{:?}", engine.sample_voice_pool.lane(lane)))
        .collect();
    let pointers: Vec<usize> = [0, 1]
        .into_iter()
        .map(|lane| {
            engine
                .sample_voice_pool
                .lane(lane)
                .and_then(|voice| voice.buffer.as_ref())
                .expect("sample lane buffer")
                .samples
                .as_ptr() as usize
        })
        .collect();

    for parity in 0..2 {
        let partition = engine
            .sample_voice_pool
            .take_partition(parity)
            .expect("active partition home");
        let address = (&*partition) as *const _ as usize;
        assert!(engine
            .sample_voice_pool
            .install_partition(parity, partition)
            .is_ok());
        let partition = engine
            .sample_voice_pool
            .take_partition(parity)
            .expect("round-trip partition home");
        assert_eq!((&*partition) as *const _ as usize, address);
        assert!(engine
            .sample_voice_pool
            .install_partition(parity, partition)
            .is_ok());
    }

    assert_eq!(
        metadata,
        (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| engine.sample_voice_pool.slot_lanes(slot).unwrap().to_vec())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        states,
        [0, 1]
            .into_iter()
            .map(|lane| format!("{:?}", engine.sample_voice_pool.lane(lane)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        pointers,
        [0, 1]
            .into_iter()
            .map(|lane| {
                engine
                    .sample_voice_pool
                    .lane(lane)
                    .and_then(|voice| voice.buffer.as_ref())
                    .expect("sample lane buffer")
                    .samples
                    .as_ptr() as usize
            })
            .collect::<Vec<_>>()
    );
    engine.sample_voice_pool.assert_invariants();
}

#[test]
fn active_synth_partition_round_trip_preserves_voice_and_metadata() {
    let mut engine = SynthEngine::new(48_000);
    engine.note_on(0, 60, 100, 2_000);
    engine.note_on(1, 72, 100, 2_000);
    let metadata: Vec<Vec<usize>> = (0..INSTRUMENT_SLOT_COUNT)
        .map(|slot| engine.synth_voice_pool.slot_lanes(slot).unwrap().to_vec())
        .collect();
    let states: Vec<String> = [0, 1]
        .into_iter()
        .map(|lane| format!("{:?}", engine.synth_voice_pool.lane(lane)))
        .collect();

    for parity in 0..2 {
        let partition = engine
            .synth_voice_pool
            .take_partition(parity)
            .expect("active partition home");
        let address = (&*partition) as *const _ as usize;
        assert!(engine
            .synth_voice_pool
            .install_partition(parity, partition)
            .is_ok());
        let partition = engine
            .synth_voice_pool
            .take_partition(parity)
            .expect("round-trip partition home");
        assert_eq!((&*partition) as *const _ as usize, address);
        assert!(engine
            .synth_voice_pool
            .install_partition(parity, partition)
            .is_ok());
    }

    assert_eq!(
        metadata,
        (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| engine.synth_voice_pool.slot_lanes(slot).unwrap().to_vec())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        states,
        [0, 1]
            .into_iter()
            .map(|lane| format!("{:?}", engine.synth_voice_pool.lane(lane)))
            .collect::<Vec<_>>()
    );
    engine.synth_voice_pool.assert_invariants();
}

fn sample_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    engine
}

fn sample_bank(value: f32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![value; 128].into(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    bank
}

#[test]
fn immediate_momentary_stop_returns_removed_state_for_retirement() {
    let mut engine = SynthEngine::new(48_000);
    engine.momentary_fx_start(
        "stutter".into(),
        "stutter".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Global,
    );

    let retired = engine.momentary_fx_stop("stutter");

    assert_eq!(engine.profile_snapshot().active_momentary_fx, 0);
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);
}

#[test]
fn render_release_completion_moves_momentary_state_to_pending_retirement() {
    let mut engine = SynthEngine::new(48_000);
    engine.momentary_fx_start(
        "filter".into(),
        "filter_sweep".into(),
        BTreeMap::from([
            ("sweepInMs".into(), Value::from(1.0)),
            ("sweepOutMs".into(), Value::from(1.0)),
        ]),
        MomentaryFxTarget::Global,
    );
    let retired = engine.momentary_fx_stop("filter");
    assert!(retired.displaced_momentary_fx.iter().all(Option::is_none));

    let _ = engine.next_stereo_sample();

    assert_eq!(engine.profile_snapshot().active_momentary_fx, 0);
    assert!(!engine.pending_render_retired_is_empty());
    let retired = engine.take_pending_render_retired();
    assert_eq!(retired.displaced_momentary_fx.iter().flatten().count(), 1);
}
