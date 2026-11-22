//! Writes every factory preset, Drum and Sampler kit and factory-patch instrument as a stereo WAV
//! for listening before and after a sound change: `OCTESSERA_RENDER_DIR=<dir> cargo test -p playback-runtime
//! factory_sound_renders -- --ignored`.

use super::*;
use crate::instrument_presets::instrument_presets;
use crate::native_menu::{FactoryLoadKind, NativeMenuAction, PlatformAction, SAMPLER_KITS};
use realtime_engine::synth::{
    normalize_instrument_slot_config, SampleBuffer, SampleSlotConfig, SynthEngine,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};
use std::path::Path;

const SAMPLE_RATE: u32 = 44_100;
const NOTES: [u8; 6] = [36, 48, 60, 69, 81, 93];
const NOTE_MS: u32 = 400;
const STEP_FRAMES: usize = SAMPLE_RATE as usize * 3 / 4;
const TAIL_FRAMES: usize = SAMPLE_RATE as usize * 3;

fn loaded_config(runner: &mut NativeRunner, kind: FactoryLoadKind, id: &str) -> Value {
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            PlatformAction::FactoryInstrumentLoad {
                kind,
                slot: 0,
                id: id.into(),
            },
        ))
        .unwrap();
    runner.instrument_audio_config(0).unwrap()
}

/// Reads a repository WAV (16, 24 or 32-bit PCM, or 32-bit float) as the hosts decode it.
fn read_sample(path: &str) -> SampleBuffer {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path),
    )
    .unwrap_or_else(|error| panic!("{path}: {error}"));
    let (mut format, mut data) = (None, None);
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        match &bytes[at..at + 4] {
            b"fmt " => format = Some(body),
            b"data" => data = Some(body),
            _ => {}
        }
        at += 8 + len + len % 2;
    }
    let (format, data) = (format.unwrap(), data.unwrap());
    let word = |at: usize| u16::from_le_bytes([format[at], format[at + 1]]);
    let tag = if word(0) == 0xfffe { word(24) } else { word(0) };
    let bits = usize::from(word(14));
    let samples: Vec<f32> = data
        .chunks_exact(bits / 8)
        .map(|bytes| match (tag, bits) {
            (3, 32) => f32::from_le_bytes(bytes.try_into().unwrap()),
            (1, _) => {
                let mut word = [0_u8; 4];
                word[4 - bytes.len()..].copy_from_slice(bytes);
                (i32::from_le_bytes(word) >> (32 - bits)) as f32 / (1_u64 << (bits - 1)) as f32
            }
            _ => panic!("{path}: unsupported WAV format {tag}/{bits}"),
        })
        .collect();
    SampleBuffer {
        samples: samples.into(),
        channels: word(2),
        sample_rate: u32::from_le_bytes(format[4..8].try_into().unwrap()),
    }
}

/// One instrument slot's engine: its config, and for a Sampler its eight slots' samples.
fn engine_for(config: &Value) -> SynthEngine {
    let normalized = normalize_instrument_slot_config(config).unwrap();
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instrument_slot(0, normalized.slot.clone());
    if let Some(sample) = normalized.active_sample() {
        let slots = (0..SAMPLE_SLOTS_PER_INSTRUMENT)
            .map(|index| SampleSlotConfig {
                buffer: sample.slots[index].as_deref().map(read_sample),
                ..Default::default()
            })
            .collect();
        let _ = engine.set_sample_bank(0, sample.bank(slots));
    }
    engine
}

fn render(config: &Value) -> Vec<f32> {
    let kind = config["type"].as_str().unwrap_or_default().to_owned();
    let tiled = matches!(kind.as_str(), "drum" | "sampler");
    let mut engine = engine_for(config);
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut stereo = Vec::new();
    let hits: Vec<u8> = if tiled {
        (0..8).collect()
    } else {
        NOTES.to_vec()
    };
    let frames = hits.len() * STEP_FRAMES + TAIL_FRAMES;
    for (step, hit) in hits.into_iter().enumerate() {
        if tiled {
            engine.tile_hit(0, hit, hit, 0, 110);
        } else {
            engine.note_on(0, hit, 110, NOTE_MS);
        }
        while stereo.len() / 2 < (step + 1) * STEP_FRAMES {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
            stereo.extend_from_slice(&out);
        }
    }
    while stereo.len() / 2 < frames {
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        stereo.extend_from_slice(&out);
    }
    stereo
}

fn write_wav(path: &Path, stereo: &[f32]) {
    let data_len = (stereo.len() * 4) as u32;
    let mut bytes = Vec::with_capacity(44 + stereo.len() * 4);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 8).to_le_bytes());
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&32_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in stereo {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
#[ignore = "writes WAV files for listening; needs OCTESSERA_RENDER_DIR"]
fn factory_sound_renders() {
    let dir = std::path::PathBuf::from(
        std::env::var("OCTESSERA_RENDER_DIR").expect("OCTESSERA_RENDER_DIR must name a directory"),
    );
    std::fs::create_dir_all(&dir).unwrap();
    let sampler_kits = SAMPLER_KITS.iter().map(|(id, ..)| (*id).to_owned());
    let families = [
        ("synth", FactoryLoadKind::SynthPreset),
        ("fm", FactoryLoadKind::FmPreset),
        ("pluck", FactoryLoadKind::PluckPreset),
        ("drum", FactoryLoadKind::DrumKit),
    ]
    .into_iter()
    .flat_map(|(family, kind)| {
        instrument_presets(family)
            .iter()
            .map(move |preset| (family, kind, preset.id.clone()))
    })
    .chain(sampler_kits.map(|id| ("sampler", FactoryLoadKind::SampleKit, id)));
    for (family, kind, id) in families {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let config = loaded_config(&mut runner, kind, &id);
        write_wav(&dir.join(format!("{family}-{id}.wav")), &render(&config));
    }
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(
            serde_json::from_str(include_str!(
                "../../../../../config/generated/pi/default.json"
            ))
            .unwrap(),
        )
        .unwrap();
    for slot in 0..8_usize {
        let config = runner.instrument_audio_config(slot).unwrap();
        let kind = config["type"].as_str().unwrap_or_default().to_owned();
        if !matches!(kind.as_str(), "synth" | "fm" | "pluck" | "drum" | "sampler") {
            continue;
        }
        let name = format!("factory-patch-i{}-{kind}.wav", slot + 1);
        write_wav(&dir.join(name), &render(&config));
    }
}
