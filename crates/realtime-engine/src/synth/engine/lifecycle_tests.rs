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
