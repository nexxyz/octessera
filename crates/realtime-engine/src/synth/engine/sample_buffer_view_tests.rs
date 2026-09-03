use super::super::runtime_state::BiquadState;
use super::super::types::SampleBuffer;
use super::source_lane_renderer::{render_sample_voice_block, render_sample_voice_frame};
use super::support::{
    mono_frame, reset_sample_buffer_view_resolves_for_test, sample_buffer_view_resolves_for_test,
    SampleBufferView, SampleVoice,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

const SAMPLE_RATE: u32 = 48_000;

#[test]
fn sample_buffer_view_matches_scalar_reads_at_one_and_two_frame_edges() {
    let cases = [
        (vec![0.25], 1),
        (vec![0.25, -0.75], 1),
        (vec![0.25, -0.75], 2),
        (vec![0.25, 0.75, -0.5, 0.5], 2),
    ];

    for (samples, channels) in cases {
        let buffer = sample_buffer(samples, channels);
        let view = SampleBufferView::from_buffer(&buffer);
        let frames = buffer.samples.len() / channels as usize;
        assert_eq!(view.frames(), frames);
        assert_eq!(view.end_position().to_bits(), (frames as f32).to_bits());
        for frame in 0..frames {
            assert_eq!(
                view.mono_frame(frame).to_bits(),
                mono_frame(&buffer, frame).to_bits()
            );
        }
    }
}

#[test]
fn prepared_sample_view_matches_scalar_render_for_fractional_unity_and_high_steps() {
    let cases = [
        (vec![0.25], 1, 0.0, 1.0, 4),
        (vec![0.25, -0.75], 2, 0.25, 0.5, 5),
        (vec![0.25, -0.75, 0.5], 1, 0.0, 3.0, 4),
        (vec![0.25, -0.75], 1, 1.0, 0.0, 2),
    ];

    for (samples, channels, pos, step, block_frames) in cases {
        assert_block_matches_scalar(
            sample_voice(samples, channels, pos, step, 3_200.0, 47.0),
            block_frames,
        );
    }
}

#[test]
fn prepared_sample_view_preserves_exact_last_frame_and_mid_block_completion() {
    let mut exact_last = sample_voice(vec![0.25, -0.75], 1, 1.0, 0.0, 3_200.0, 47.0);
    let mut exact_last_scalar = exact_last.clone();
    let (output, active) = render_block(&mut exact_last, 2);
    let (scalar_output, scalar_active) = render_scalar(&mut exact_last_scalar, 2);
    assert!(active.iter().all(|is_active| *is_active));
    assert_eq!(active, scalar_active);
    for (actual, expected) in output.iter().zip(scalar_output) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(exact_last.pos.to_bits(), 1.0_f32.to_bits());
    assert!(exact_last.active);
    assert_eq!(exact_last.filt, exact_last_scalar.filt);

    let mut block = sample_voice(vec![1.0, -0.25, 0.5], 1, 0.0, 1.0, 2_400.0, 68.0);
    let mut scalar = block.clone();
    let (block_output, block_active) = render_block(&mut block, 8);
    let (scalar_output, scalar_active) = render_scalar(&mut scalar, 8);
    assert_eq!(block_active, scalar_active);
    for (actual, expected) in block_output.iter().zip(scalar_output) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(block.pos.to_bits(), 3.0_f32.to_bits());
    assert!(!block.active);
    assert_eq!(block.filt, scalar.filt);
}

#[test]
fn malformed_and_empty_sample_geometry_matches_scalar_failure() {
    assert_block_matches_scalar(sample_voice(Vec::new(), 1, 0.0, 1.0, 3_200.0, 47.0), 4);
    assert_block_matches_scalar(sample_voice(vec![0.25], 2, 0.0, 1.0, 3_200.0, 47.0), 4);

    let block_panic = catch_unwind(AssertUnwindSafe(|| {
        let mut voice = sample_voice(vec![0.25], 0, 0.0, 1.0, 3_200.0, 47.0);
        let _ = render_block(&mut voice, 1);
    }));
    let scalar_panic = catch_unwind(AssertUnwindSafe(|| {
        let mut voice = sample_voice(vec![0.25], 0, 0.0, 1.0, 3_200.0, 47.0);
        let _ = render_scalar(&mut voice, 1);
    }));
    assert_eq!(block_panic.is_err(), scalar_panic.is_err());
    assert!(block_panic.is_err());
}

#[test]
fn sample_buffer_view_resolves_once_per_voice_block_not_per_frame() {
    let mut voice = sample_voice(vec![0.25; 4_096], 1, 0.0, 1.0, 3_200.0, 47.0);
    reset_sample_buffer_view_resolves_for_test();
    let _ = render_block(&mut voice, 2_048);
    assert_eq!(sample_buffer_view_resolves_for_test(), 1);

    reset_sample_buffer_view_resolves_for_test();
    let mut scalar = sample_voice(vec![0.25; 4_096], 1, 0.0, 1.0, 3_200.0, 47.0);
    let _ = render_scalar(&mut scalar, 2_048);
    assert_eq!(sample_buffer_view_resolves_for_test(), 0);
}

#[test]
fn sample_buffer_view_borrows_pcm_without_changing_arc_ownership() {
    let mut voice = sample_voice(vec![0.25; 64], 1, 0.0, 1.0, 3_200.0, 47.0);
    let samples = voice
        .buffer
        .as_ref()
        .expect("sample buffer")
        .samples
        .clone();
    let before = std::sync::Arc::strong_count(&samples);
    let _ = render_block(&mut voice, 8);
    assert_eq!(std::sync::Arc::strong_count(&samples), before);
    assert!(std::sync::Arc::ptr_eq(
        &voice.buffer.expect("sample buffer").samples,
        &samples
    ));
}

fn assert_block_matches_scalar(voice: SampleVoice, frames: usize) {
    let mut block = voice.clone();
    let mut scalar = voice;
    let (block_output, block_active) = render_block(&mut block, frames);
    let (scalar_output, scalar_active) = render_scalar(&mut scalar, frames);
    assert_eq!(block_active, scalar_active);
    for (index, (actual, expected)) in block_output.iter().zip(scalar_output).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
    }
    assert_eq!(block.pos.to_bits(), scalar.pos.to_bits());
    assert_eq!(block.active, scalar.active);
    assert_eq!(block.filt, scalar.filt);
}

fn render_block(voice: &mut SampleVoice, frames: usize) -> (Vec<f32>, Vec<bool>) {
    let mut output = vec![0.0; frames];
    let rendered_frames = render_sample_voice_block(voice, frames, SAMPLE_RATE, &mut output);
    let mut active = vec![false; frames];
    active[..rendered_frames].fill(true);
    (output, active)
}

fn render_scalar(voice: &mut SampleVoice, frames: usize) -> (Vec<f32>, Vec<bool>) {
    let mut output = vec![0.0; frames];
    let mut active = vec![false; frames];
    for frame in 0..frames {
        if let Some(sample) = render_sample_voice_frame(voice, SAMPLE_RATE) {
            output[frame] = sample;
            active[frame] = true;
        }
    }
    (output, active)
}

fn sample_voice(
    samples: Vec<f32>,
    channels: u16,
    pos: f32,
    step: f32,
    cutoff_hz: f32,
    resonance: f32,
) -> SampleVoice {
    SampleVoice {
        active: true,
        canonical_lane: None,
        instrument_slot: 0,
        sample_slot: 0,
        buffer: Some(sample_buffer(samples, channels)),
        filter_cutoff_hz: cutoff_hz,
        filter_resonance: resonance,
        pos,
        step,
        gain: 0.75,
        filt: BiquadState::new(),
    }
}

fn sample_buffer(samples: Vec<f32>, channels: u16) -> SampleBuffer {
    SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate: SAMPLE_RATE,
    }
}
