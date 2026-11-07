use super::*;
use crate::synth::{
    InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig, SampleBuffer, SampleLoopMode,
    SamplePlaybackConfig,
};

const RATE: u32 = 48_000;

fn sampler_engine(banks: Vec<SampleBankConfig>) -> SynthEngine {
    let mut engine = SynthEngine::new(RATE);
    engine.set_instruments(InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|_| InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
                kind: "sampler".to_string(),
                synth: default_synth_config(),
                mixer: None,
            })
            .collect(),
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let _ = engine.set_sample_banks(banks);
    engine
}

fn tone(frames: usize, channels: u16, sample_rate: u32) -> SampleBuffer {
    let samples = (0..frames * usize::from(channels))
        .map(|index| {
            let frame = (index / usize::from(channels)) as f32;
            let channel = (index % usize::from(channels)) as f32;
            (frame * 0.031 + channel).sin() * 0.6 + (frame * 0.17).sin() * 0.2
        })
        .collect::<Vec<_>>();
    SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate,
    }
}

fn ramp(frames: usize) -> SampleBuffer {
    SampleBuffer {
        samples: (0..frames)
            .map(|index| index as f32 / frames as f32)
            .collect::<Vec<_>>()
            .into(),
        channels: 1,
        sample_rate: RATE,
    }
}

fn sine(frames: usize, period: f32) -> SampleBuffer {
    SampleBuffer {
        samples: (0..frames)
            .map(|index| (index as f32 * std::f32::consts::TAU / period).sin() * 0.5)
            .collect::<Vec<_>>()
            .into(),
        channels: 1,
        sample_rate: RATE,
    }
}

fn playback(
    loop_mode: SampleLoopMode,
    start: f32,
    loop_start: f32,
    loop_end: f32,
) -> SamplePlaybackConfig {
    SamplePlaybackConfig {
        loop_mode,
        start_pct: start,
        loop_start_pct: loop_start,
        loop_end_pct: loop_end,
    }
}

fn looping_engine(buffer: SampleBuffer, playback: SamplePlaybackConfig) -> SynthEngine {
    let mut bank = SampleBankConfig::default();
    bank.slots[0].buffer = Some(buffer);
    bank.slots[0].playback = playback;
    let mut banks = vec![bank];
    banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
    let mut engine = sampler_engine(banks);
    engine.note_on(0, 36, 127, 1_000);
    engine
}

fn render_mono(engine: &mut SynthEngine, frames: usize) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut mono = Vec::with_capacity(frames);
    while mono.len() < frames {
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        mono.extend(
            left.iter()
                .zip(&right)
                .map(|(left, right)| (left + right) * 0.5),
        );
    }
    mono.truncate(frames);
    mono
}

fn max_step(output: &[f32]) -> f32 {
    output
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f32::max)
}

/// The engine's gain from a ramp value to the mono output, read off the untouched first pass.
fn ramp_scale(output: &[f32], frames: usize) -> f32 {
    output[frames / 10] / 0.1
}

fn whole_ramp_scale(frames: usize) -> f32 {
    let mut whole = looping_engine(ramp(frames), playback(SampleLoopMode::Off, 0.0, 0.0, 100.0));
    ramp_scale(&render_mono(&mut whole, frames), frames)
}

#[test]
fn forward_loop_crossfades_the_seam_and_keeps_playing_past_the_sample() {
    let frames = 4_800;
    let mut engine = looping_engine(
        sine(frames, 333.3),
        playback(SampleLoopMode::Forward, 0.0, 0.0, 100.0),
    );
    let output = render_mono(&mut engine, frames * 4);
    let plain_step = max_step(&output[..4_000]);
    assert!(plain_step > 0.0);
    assert!(
        max_step(&output) <= plain_step * 1.5,
        "seam step {} against plain {plain_step}",
        max_step(&output)
    );
    assert!(output[frames * 4 - 400..]
        .iter()
        .any(|value| value.abs() > plain_step));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
}

#[test]
fn forward_loop_without_room_to_crossfade_still_wraps_inside_its_points() {
    let frames = 1_000;
    let scale = whole_ramp_scale(frames);
    let mut engine = looping_engine(
        ramp(frames),
        playback(SampleLoopMode::Forward, 0.0, 40.0, 41.0),
    );
    let output = render_mono(&mut engine, frames * 3);
    for value in &output[frames..] {
        let at = value / scale;
        assert!((0.399..=0.411).contains(&at), "{at}");
    }
}

#[test]
fn ping_pong_turns_around_at_both_loop_points_without_a_step() {
    let frames = 1_000;
    let mut engine = looping_engine(
        ramp(frames),
        playback(SampleLoopMode::PingPong, 0.0, 0.0, 100.0),
    );
    let output = render_mono(&mut engine, frames * 3);
    let scale = ramp_scale(&output, frames);
    let ramp_step = scale / frames as f32;
    assert!(max_step(&output) <= ramp_step * 1.01);
    let peak = (0..1_500)
        .max_by(|left, right| output[*left].total_cmp(&output[*right]))
        .unwrap();
    assert!((990..=1_010).contains(&peak), "turns at {peak}");
    assert!(output[peak + 500] < output[peak + 100]);
    let trough = peak + 999;
    assert!(output[trough] < scale * 0.01, "{}", output[trough] / scale);
    assert!(output[trough + 200] > output[trough + 100]);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
}

#[test]
fn loops_stay_between_loop_start_and_loop_end() {
    let frames = 1_000;
    let scale = whole_ramp_scale(frames);
    for mode in [SampleLoopMode::Forward, SampleLoopMode::PingPong] {
        let mut engine = looping_engine(ramp(frames), playback(mode, 0.0, 25.0, 75.0));
        let output = render_mono(&mut engine, frames * 4);
        for value in &output[frames..] {
            let at = value / scale;
            assert!((0.249..=0.751).contains(&at), "{mode:?} reads {at}");
        }
    }
}

#[test]
fn start_offset_skips_the_front_of_a_oneshot_sample() {
    let frames = 1_000;
    let scale = whole_ramp_scale(frames);
    let mut engine = looping_engine(
        ramp(frames),
        playback(SampleLoopMode::Off, 50.0, 0.0, 100.0),
    );
    let output = render_mono(&mut engine, frames);
    assert!(
        (output[0] / scale - 0.5).abs() < 0.002,
        "{}",
        output[0] / scale
    );
    assert!(output[490] > 0.0);
    assert!(output[520..].iter().all(|value| *value == 0.0));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}

#[test]
fn a_looping_start_past_loop_end_begins_at_loop_start() {
    let frames = 1_000;
    let scale = whole_ramp_scale(frames);
    let mut engine = looping_engine(
        ramp(frames),
        playback(SampleLoopMode::Forward, 90.0, 20.0, 60.0),
    );
    let output = render_mono(&mut engine, 200);
    assert!(
        (output[0] / scale - 0.2).abs() < 0.002,
        "{}",
        output[0] / scale
    );
}

#[test]
fn releasing_a_looping_note_fades_it_out_with_the_amp_envelope() {
    let frames = 1_000;
    for mode in [SampleLoopMode::Forward, SampleLoopMode::PingPong] {
        let mut engine = looping_engine(ramp(frames), playback(mode, 0.0, 0.0, 100.0));
        let held = render_mono(&mut engine, frames * 5);
        assert!(held[held.len() - 64..].iter().any(|value| *value != 0.0));
        engine.note_off(0, 36);
        let released = render_mono(&mut engine, 1_024);
        assert!(released[..64].iter().any(|value| *value != 0.0));
        assert!(
            released[512..].iter().all(|value| *value == 0.0),
            "{mode:?}"
        );
        assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
    }
}

#[test]
fn loop_cursors_render_the_same_on_the_block_and_frame_paths() {
    for mode in [SampleLoopMode::Forward, SampleLoopMode::PingPong] {
        for attack_ms in [0.0, 20.0] {
            let build = || {
                let mut bank = SampleBankConfig::default();
                bank.slots[0].buffer = Some(tone(900, 2, 44_100));
                bank.slots[0].playback = playback(mode, 10.0, 30.0, 80.0);
                bank.tune_semis = 5.0;
                bank.shape.amp_env.attack_ms = attack_ms;
                let mut banks = vec![bank];
                banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
                let mut engine = sampler_engine(banks);
                engine.note_on(0, 36, 100, 1_000);
                engine
            };
            let mut block = build();
            let mut frame = build();
            let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
            for _ in 0..40 {
                block.render_interleaved_block(128, &mut left, &mut right, &mut out);
                for pair in out.chunks(2) {
                    let (expected_left, expected_right) = frame.next_stereo_sample();
                    assert_eq!(pair[0].to_bits(), expected_left.to_bits(), "{mode:?}");
                    assert_eq!(pair[1].to_bits(), expected_right.to_bits(), "{mode:?}");
                }
            }
            assert_eq!(block.profile_snapshot().active_sample_voices, 1);
        }
    }
}

#[test]
fn looping_voices_render_without_allocating() {
    let mut bank = SampleBankConfig::default();
    bank.slots[0].buffer = Some(tone(2_000, 1, RATE));
    bank.slots[0].playback = playback(SampleLoopMode::Forward, 0.0, 10.0, 90.0);
    bank.slots[1].buffer = Some(tone(2_000, 2, 22_050));
    bank.slots[1].playback = playback(SampleLoopMode::PingPong, 5.0, 10.0, 90.0);
    let mut banks = vec![bank];
    banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
    let mut engine = sampler_engine(banks);
    engine.note_on(0, 36, 100, 1_000);
    engine.tile_hit(0, 1, 3, 700, 100);
    let mut left = Vec::with_capacity(128);
    let mut right = Vec::with_capacity(128);
    let mut out = Vec::with_capacity(256);
    engine.render_interleaved_block(128, &mut left, &mut right, &mut out);

    let (_, allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for _ in 0..200 {
                engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
            }
        });
    assert_eq!(allocations, 0);
    assert_eq!(deallocations, 0);
    assert_eq!(engine.profile_snapshot().active_sample_voices, 2);
}

fn normalized_slot(note_behavior: &str) -> SamplePlaybackConfig {
    let instrument = crate::synth::normalize_instrument_slot_config(&serde_json::json!({
        "type": "sampler",
        "noteBehavior": note_behavior,
        "sample": { "slots": [{
            "path": "a.wav",
            "loop": "pingpong",
            "startPct": 12,
            "loopStartPct": 20,
            "loopEndPct": 70
        }] }
    }))
    .expect("sampler payload");
    instrument.active_sample().expect("sample").slot_playback[0]
}

#[test]
fn only_held_notes_loop_and_start_applies_in_both_modes() {
    assert_eq!(
        normalized_slot("hold"),
        playback(SampleLoopMode::PingPong, 12.0, 20.0, 70.0)
    );
    assert_eq!(
        normalized_slot("oneshot"),
        playback(SampleLoopMode::Off, 12.0, 20.0, 70.0)
    );
}

#[test]
fn oneshot_slots_play_once_even_when_a_loop_is_saved() {
    let instrument = crate::synth::normalize_instrument_slot_config(&serde_json::json!({
        "type": "sampler",
        "noteBehavior": "oneshot",
        "sample": { "slots": [{ "path": "a.wav", "loop": "forward" }] }
    }))
    .expect("sampler payload");
    let mut slots = vec![crate::synth::SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
    slots[0].buffer = Some(ramp(1_000));
    let mut banks = vec![instrument.active_sample().expect("sample").bank(slots)];
    banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
    let mut engine = sampler_engine(banks);
    engine.note_on(0, 36, 127, 1_000);
    let output = render_mono(&mut engine, 2_000);
    assert!(output[1_010..].iter().all(|value| *value == 0.0));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}

#[test]
fn loop_end_must_stay_above_loop_start() {
    let error = crate::synth::normalize_instrument_slot_config(&serde_json::json!({
        "type": "sampler",
        "noteBehavior": "hold",
        "sample": { "slots": [{ "path": null, "loopStartPct": 50, "loopEndPct": 50 }] }
    }))
    .expect_err("empty loop");
    assert!(error.contains("Loop End"), "{error}");
}
