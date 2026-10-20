use super::*;
use crate::native_menu::NativeMenuAction;
use realtime_engine::synth::{InstrumentSlotConfig, SynthEngine};

const SAMPLE_RATE: u32 = 48_000;
const NOTES: [u8; 5] = [36, 48, 60, 72, 84];
const NOTE_MS: u32 = 300;
const STEP_FRAMES: usize = 24_000;
const TAIL_FRAMES: usize = SAMPLE_RATE as usize * 3;

fn factory_slot(action: &str) -> InstrumentSlotConfig {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(action.into()))
        .unwrap();
    serde_json::from_value(runner.instrument_audio_config(0).unwrap())
        .unwrap_or_else(|error| panic!("{action}: {error}"))
}

fn render(slot: InstrumentSlotConfig, drum: bool) -> (Vec<f32>, Vec<usize>) {
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instrument_slot(0, slot);
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut mono = Vec::new();
    let mut onsets = Vec::new();
    let hits: Vec<u8> = if drum {
        (0..8).collect()
    } else {
        NOTES.to_vec()
    };
    for (step, hit) in hits.into_iter().enumerate() {
        onsets.push(mono.len());
        if drum {
            engine.cell_hit(0, hit, hit, 0, 110);
        } else {
            engine.note_on(0, hit, 110, NOTE_MS);
        }
        while mono.len() < (step + 1) * STEP_FRAMES {
            engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
            mono.extend(
                out.chunks(2)
                    .map(|frame| frame[0].abs().max(frame[1].abs())),
            );
        }
    }
    let end = mono.len() + TAIL_FRAMES;
    while mono.len() < end {
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        mono.extend(
            out.chunks(2)
                .map(|frame| frame[0].abs().max(frame[1].abs())),
        );
    }
    (mono, onsets)
}

fn assert_sane(action: &str, drum: bool) {
    let (peaks, onsets) = render(factory_slot(action), drum);
    assert!(
        peaks.iter().all(|value| value.is_finite()),
        "{action}: non-finite output"
    );
    let loudest = peaks.iter().copied().fold(0.0_f32, f32::max);
    assert!(loudest < 1.0, "{action}: clips at {loudest}");
    for onset in onsets {
        let early = peaks[onset..onset + SAMPLE_RATE as usize / 20]
            .iter()
            .copied()
            .fold(0.0_f32, f32::max);
        assert!(early > 1.0e-3, "{action}: silent note at frame {onset}");
    }
    let tail = &peaks[peaks.len() - SAMPLE_RATE as usize / 10..];
    assert!(
        tail.iter().all(|value| *value < 1.0e-4),
        "{action}: still sounding three seconds after the last note"
    );
}

#[test]
fn every_factory_sound_plays_stays_below_clipping_and_ends() {
    for preset in [
        "init",
        "soft_pad",
        "bright_pluck",
        "bass_mono",
        "hollow_pwm",
        "lead",
        "bell",
        "perc_hit",
    ] {
        assert_sane(&format!("synth.preset:0:{preset}"), false);
    }
    for preset in ["init", "soft_keys", "bell"] {
        assert_sane(&format!("fm.preset:0:{preset}"), false);
    }
    for preset in ["init", "nylon", "steel", "muted"] {
        assert_sane(&format!("pluck.preset:0:{preset}"), false);
    }
    for kit in ["default", "tight", "heavy"] {
        assert_sane(&format!("drum.kit:0:{kit}"), true);
    }
}
