//! Default slot playback must render exactly as the sampler did before slots could loop. The
//! voices read deterministic noise through an open filter, so the hashes hold on every platform.

use super::super::runtime_state::BiquadState;
use super::super::types::{EnvConfig, SampleBuffer, SamplePlaybackConfig, SampleShapeConfig};
use super::sample_voice::SampleVoice;
use super::sample_voice_loop::SampleVoiceLoop;
use super::sample_voice_renderer::{render_sample_voice_block, render_sample_voice_frame};

const IDENTITY_RATE: u32 = 48_000;

/// Deterministic noise, so the renders use no platform maths before the voice.
fn identity_buffer(frames: usize, channels: u16, seed: u32) -> SampleBuffer {
    let mut state = seed;
    let samples = (0..frames * usize::from(channels))
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 8) as f32 / (1 << 23) as f32 - 1.0
        })
        .collect::<Vec<_>>();
    SampleBuffer {
        samples: samples.into(),
        channels,
        sample_rate: IDENTITY_RATE,
    }
}

fn identity_shape(attack_ms: f32) -> SampleShapeConfig {
    let mut shape = SampleShapeConfig::default();
    if attack_ms > 0.0 {
        shape.amp_env = EnvConfig {
            attack_ms,
            decay_ms: 50.0,
            sustain_pct: 60.0,
            release_ms: 30.0,
        };
    }
    shape
}

fn identity_voices() -> Vec<(SampleVoice, SampleShapeConfig)> {
    [
        (3_000, 1, 22_050.0 / 48_000.0, 0.0, 0),
        (2_000, 2, 1.0, 0.0, 37),
        (4_000, 2, 44_100.0 / 48_000.0, 10.0, 0),
        (2_500, 1, 1.5, 25.0, 300),
    ]
    .into_iter()
    .enumerate()
    .map(
        |(index, (frames, channels, step, attack_ms, start_delay))| {
            let shape = identity_shape(attack_ms);
            let buffer = identity_buffer(frames, channels, index as u32 + 7);
            (identity_voice(buffer, &shape, step, start_delay), shape)
        },
    )
    .collect()
}

fn fnv(hash: u64, value: f32) -> u64 {
    (hash ^ u64::from(value.to_bits())).wrapping_mul(0x0000_0100_0000_01b3)
}

fn identity_hash_blocks() -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for (mut voice, shape) in identity_voices() {
        let mut block = vec![0.0; 128];
        for index in 0..64 {
            if index == 20 {
                voice.shape.release_after(&shape, 77, IDENTITY_RATE);
            }
            block.fill(0.0);
            let rendered = render_sample_voice_block(&mut voice, 128, IDENTITY_RATE, &mut block);
            hash = block[..rendered]
                .iter()
                .fold(fnv(hash, rendered as f32), |hash, value| fnv(hash, *value));
        }
    }
    hash
}

fn identity_hash_frames() -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325;
    for (mut voice, shape) in identity_voices() {
        for frame in 0..8_192 {
            if frame == 2_637 {
                voice.shape.release_after(&shape, 0, IDENTITY_RATE);
            }
            let value = if voice.start_delay > 0 {
                voice.start_delay -= 1;
                0.0
            } else {
                render_sample_voice_frame(&mut voice, IDENTITY_RATE).unwrap_or(-9.0)
            };
            hash = fnv(hash, value);
        }
    }
    hash
}

fn identity_voice(
    buffer: SampleBuffer,
    shape: &SampleShapeConfig,
    step: f32,
    start_delay: u32,
) -> SampleVoice {
    let (cursor, pos) = SampleVoiceLoop::start(&SamplePlaybackConfig::default(), &buffer);
    SampleVoice {
        active: true,
        instrument_slot: 0,
        sample_slot: 0,
        tile: None,
        buffer: Some(buffer),
        filter_cutoff_hz: 20_000.0,
        filter_resonance: 0.0,
        pos,
        step,
        cursor,
        gain: 0.8,
        filt: BiquadState::new(),
        shape: super::sample_voice_shape::SampleVoiceShape::start(shape, 0.0, IDENTITY_RATE),
        last_output: 0.0,
        declick: Default::default(),
        start_delay,
    }
}

/// `identity_hash_blocks` and `identity_hash_frames` as rendered by the pre-loop engine at
/// b00241fa, on Windows and on x86_64 Linux alike.
const PRE_LOOP_BLOCK_HASH: u64 = 0x26f0_9692_6a98_8ae3;
const PRE_LOOP_FRAME_HASH: u64 = 0x5ddd_1cc7_5005_fd99;

#[test]
fn default_playback_renders_bit_identically_to_the_pre_loop_engine() {
    assert_eq!(identity_hash_blocks(), PRE_LOOP_BLOCK_HASH);
    assert_eq!(identity_hash_frames(), PRE_LOOP_FRAME_HASH);
}
