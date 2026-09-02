use super::super::runtime_state::{
    prepare_count_for_test, reset_prepare_count_for_test, BiquadState,
};
use super::super::types::SampleBuffer;
use super::source_lane_renderer::{render_sample_voice_block, render_sample_voice_frame};
use super::support::SampleVoice;

const SAMPLE_RATE: u32 = 48_000;

#[test]
fn prepared_sample_block_matches_scalar_for_mono_stereo_and_steps() {
    let cases = [
        (vec![1.0, -0.5, 0.25, 0.75, -0.25, 0.5], 1, 0.0, 1.0),
        (
            vec![1.0, 0.5, -0.5, 0.25, 0.75, -0.25, -0.25, 0.5],
            2,
            0.0,
            1.0,
        ),
        (vec![0.25, -0.75, 0.5, 0.125, -0.375], 1, 0.25, 0.5),
        (
            vec![0.25, 0.75, -0.5, 0.5, 0.125, -0.25, -0.375, 0.25],
            2,
            0.5,
            0.75,
        ),
    ];

    for (samples, channels, pos, step) in cases {
        assert_block_matches_scalar(sample_voice(samples, channels, pos, step, 3_200.0, 47.0), 4);
    }
}

#[test]
fn prepared_sample_block_matches_scalar_across_filter_changes_between_blocks() {
    let initial = sample_voice(
        (0..64).map(|index| (index as f32 * 0.17).sin()).collect(),
        1,
        0.0,
        0.5,
        7_000.0,
        12.0,
    );
    let mut block = initial.clone();
    let mut scalar = initial;

    for (cutoff, resonance) in [(7_000.0, 12.0), (240.0, 83.0), (1_900.0, 4.0)] {
        block.filter_cutoff_hz = cutoff;
        block.filter_resonance = resonance;
        scalar.filter_cutoff_hz = cutoff;
        scalar.filter_resonance = resonance;
        assert_block_and_scalar_state_match(&mut block, &mut scalar, 8);
    }
}

#[test]
fn prepared_sample_block_preserves_end_of_buffer_mid_block() {
    assert_block_matches_scalar(
        sample_voice(vec![1.0, -0.25, 0.5], 1, 0.0, 1.0, 2_400.0, 68.0),
        8,
    );
}

#[test]
fn prepared_filter_coefficients_affect_the_first_next_block_frame() {
    let initial = sample_voice(vec![1.0, 0.0, 0.0, 0.0], 1, 0.0, 1.0, 18_000.0, 20.0);
    let mut changed = initial.clone();
    let mut unchanged = initial;

    render_block(&mut changed, 1);
    render_block(&mut unchanged, 1);
    changed.filter_cutoff_hz = 200.0;

    let changed_output = render_block(&mut changed, 1).0;
    let unchanged_output = render_block(&mut unchanged, 1).0;
    assert_ne!(changed_output[0].to_bits(), unchanged_output[0].to_bits());
}

#[test]
fn prepared_sample_block_prepares_once_per_active_voice_block() {
    let first = sample_voice(vec![1.0; 64], 1, 0.0, 1.0, 2_000.0, 51.0);
    let mut block_first = first.clone();
    let mut block_second = first;
    reset_prepare_count_for_test();
    render_block(&mut block_first, 8);
    render_block(&mut block_second, 8);
    assert_eq!(prepare_count_for_test(), 2);

    let mut scalar_first = block_first.clone();
    let mut scalar_second = block_second.clone();
    scalar_first.pos = 0.0;
    scalar_second.pos = 0.0;
    scalar_first.filt = BiquadState::new();
    scalar_second.filt = BiquadState::new();
    reset_prepare_count_for_test();
    for _ in 0..8 {
        let _ = render_sample_voice_frame(&mut scalar_first, SAMPLE_RATE);
        let _ = render_sample_voice_frame(&mut scalar_second, SAMPLE_RATE);
    }
    assert_eq!(prepare_count_for_test(), 16);
}

fn assert_block_matches_scalar(voice: SampleVoice, frames: usize) {
    let mut block = voice.clone();
    let mut scalar = voice;
    assert_block_and_scalar_state_match(&mut block, &mut scalar, frames);
}

fn assert_block_and_scalar_state_match(
    block: &mut SampleVoice,
    scalar: &mut SampleVoice,
    frames: usize,
) {
    let (block_output, block_active) = render_block(block, frames);
    let (scalar_output, scalar_active) = render_scalar(scalar, frames);
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
    let mut active = vec![false; frames];
    render_sample_voice_block(voice, frames, SAMPLE_RATE, &mut output, &mut active);
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
        instrument_slot: 0,
        sample_slot: 0,
        buffer: Some(SampleBuffer {
            samples: samples.into(),
            channels,
            sample_rate: SAMPLE_RATE,
        }),
        filter_cutoff_hz: cutoff_hz,
        filter_resonance: resonance,
        pos,
        step,
        gain: 0.75,
        filt: BiquadState::new(),
    }
}
