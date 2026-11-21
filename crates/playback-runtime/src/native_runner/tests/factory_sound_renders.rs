//! Writes every factory preset, kit and factory-patch instrument as a stereo WAV for listening
//! before and after a sound change: `OCTESSERA_RENDER_DIR=<dir> cargo test -p playback-runtime
//! factory_sound_renders -- --ignored`.

use super::*;
use crate::instrument_presets::instrument_presets;
use crate::native_menu::{FactoryLoadKind, NativeMenuAction, PlatformAction};
use realtime_engine::synth::{InstrumentSlotConfig, SynthEngine};
use std::path::Path;

const SAMPLE_RATE: u32 = 44_100;
const NOTES: [u8; 6] = [36, 48, 60, 69, 81, 93];
const NOTE_MS: u32 = 400;
const STEP_FRAMES: usize = SAMPLE_RATE as usize * 3 / 4;
const TAIL_FRAMES: usize = SAMPLE_RATE as usize * 3;

fn loaded_slot(runner: &mut NativeRunner, kind: FactoryLoadKind, id: &str) -> InstrumentSlotConfig {
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            PlatformAction::FactoryInstrumentLoad {
                kind,
                slot: 0,
                id: id.into(),
            },
        ))
        .unwrap();
    serde_json::from_value(runner.instrument_audio_config(0).unwrap()).unwrap()
}

fn render(slot: InstrumentSlotConfig, drum: bool) -> Vec<f32> {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instrument_slot(0, slot);
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut stereo = Vec::new();
    let hits: Vec<u8> = if drum {
        (0..8).collect()
    } else {
        NOTES.to_vec()
    };
    let frames = hits.len() * STEP_FRAMES + TAIL_FRAMES;
    for (step, hit) in hits.into_iter().enumerate() {
        if drum {
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
    for (family, kind) in [
        ("synth", FactoryLoadKind::SynthPreset),
        ("fm", FactoryLoadKind::FmPreset),
        ("pluck", FactoryLoadKind::PluckPreset),
        ("drum", FactoryLoadKind::DrumKit),
    ] {
        for preset in instrument_presets(family) {
            let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
            let slot = loaded_slot(&mut runner, kind, &preset.id);
            let stereo = render(slot, kind == FactoryLoadKind::DrumKit);
            write_wav(&dir.join(format!("{family}-{}.wav", preset.id)), &stereo);
        }
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
        let config: InstrumentSlotConfig =
            serde_json::from_value(runner.instrument_audio_config(slot).unwrap()).unwrap();
        if !matches!(config.kind.as_str(), "synth" | "fm" | "pluck" | "drum") {
            continue;
        }
        let name = format!("factory-patch-i{}-{}.wav", slot + 1, config.kind);
        let drum = config.kind == "drum";
        write_wav(&dir.join(name), &render(config, drum));
    }
}
