use super::*;
use crate::native_runner::aux_auto_map::{aux_auto_map, AuxAutoMap};

pub(super) fn instrument_filter_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    ["synth", "fm", "pluck", "drum", "sample"]
        .into_iter()
        .find(|kind| field.starts_with(&format!("{kind}.filter.")))
        .map(|kind| filter_auto_map(runner, &format!("{prefix}.{kind}.filter")))
}

fn filter_auto_map(runner: &NativeRunner, base: &str) -> AuxAutoMap {
    aux_auto_map([
        Some(runner.turn_slot(format!("{base}.cutoffHz"), "Cutoff")),
        Some(runner.turn_slot(format!("{base}.resonance"), "Res")),
        Some(runner.turn_slot(format!("{base}.envAmountPct"), "Env")),
        Some(runner.turn_slot(format!("{base}.keyTrackingPct"), "Key")),
        Some(runner.turn_slot(format!("{base}.type"), "Type")),
    ])
}

pub(super) fn instrument_envelope_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if field.starts_with("synth.ampEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.synth.ampEnv")))
    } else if field.starts_with("synth.filterEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.synth.filterEnv")))
    } else if field.starts_with("fm.indexEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.fm.indexEnv")))
    } else if field.starts_with("fm.ampEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.fm.ampEnv")))
    } else if field.starts_with("fm.filterEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.fm.filterEnv")))
    } else if field.starts_with("pluck.ampEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.pluck.ampEnv")))
    } else if field.starts_with("pluck.filterEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.pluck.filterEnv")))
    } else if field.starts_with("sample.ampEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.sample.ampEnv")))
    } else if field.starts_with("sample.filterEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.sample.filterEnv")))
    } else {
        None
    }
}

pub(super) fn instrument_oscillator_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if field.starts_with("synth.osc1.") {
        Some(runner.osc_auto_map(&format!("{prefix}.synth.osc1")))
    } else if field.starts_with("synth.osc2.") {
        Some(runner.osc_auto_map(&format!("{prefix}.synth.osc2")))
    } else {
        None
    }
}

pub(super) fn instrument_amp_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    ["synth", "sample", "fm", "pluck", "drum"]
        .into_iter()
        .find(|kind| field.starts_with(&format!("{kind}.amp.")))
        .map(|kind| {
            aux_auto_map([
                Some(runner.turn_slot(format!("{prefix}.{kind}.amp.gainPct"), "Gain")),
                Some(
                    runner.turn_slot(format!("{prefix}.{kind}.amp.velocitySensitivityPct"), "Vel"),
                ),
            ])
        })
}

pub(super) fn instrument_drum_voice_auto_map(
    runner: &NativeRunner,
    index: usize,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    let voice = if field == "drum.voice" {
        *runner.drum_selected_voices.get(index)?
    } else {
        field
            .strip_prefix("drum.voices.")?
            .split_once('.')?
            .0
            .parse::<usize>()
            .ok()?
    };
    if voice >= 8 {
        return None;
    }
    let base = format!("{prefix}.drum.voices.{voice}");
    Some(aux_auto_map([
        Some(runner.turn_slot(format!("{base}.tuneSemis"), "Tune")),
        Some(runner.turn_slot(format!("{base}.decayMs"), "Decay")),
        Some(runner.turn_slot(format!("{base}.tonePct"), "Tone")),
        Some(runner.turn_slot(format!("{base}.levelPct"), "Level")),
        Some(runner.turn_slot(format!("{base}.noiseMixPct"), "Noise")),
        Some(runner.turn_slot(format!("{base}.attackMs"), "Attack")),
    ]))
}

pub(super) fn instrument_fm_tone_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if !field.starts_with("fm.") {
        return None;
    }
    Some(aux_auto_map([
        Some(runner.turn_slot(format!("{prefix}.fm.index"), "Index")),
        Some(runner.turn_slot(format!("{prefix}.fm.ratio"), "Ratio")),
        Some(runner.turn_slot(format!("{prefix}.fm.filter.cutoffHz"), "Cutoff")),
        Some(runner.turn_slot(format!("{prefix}.fm.filter.resonance"), "Res")),
        Some(runner.turn_slot(format!("{prefix}.fm.ratioFineCents"), "Fine")),
        Some(runner.turn_slot(format!("{prefix}.fm.amp.gainPct"), "Gain")),
    ]))
}

pub(super) fn instrument_pluck_string_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if !field.starts_with("pluck.") {
        return None;
    }
    Some(aux_auto_map([
        Some(runner.turn_slot(format!("{prefix}.pluck.decayMs"), "Decay")),
        Some(runner.turn_slot(format!("{prefix}.pluck.brightnessPct"), "Bright")),
        Some(runner.turn_slot(format!("{prefix}.pluck.pickPositionPct"), "Pick")),
        Some(runner.turn_slot(format!("{prefix}.pluck.bodyAmountPct"), "Body")),
        Some(runner.turn_slot(format!("{prefix}.pluck.dispersionPct"), "Disp")),
        Some(runner.turn_slot(format!("{prefix}.pluck.amp.gainPct"), "Gain")),
    ]))
}

pub(super) fn instrument_sample_auto_map(
    runner: &NativeRunner,
    index: usize,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if !field.starts_with("sample.") {
        return None;
    }
    let sample_slot = runner
        .instruments
        .get(index)
        .map(|instrument| instrument.selected_sample_slot.min(SAMPLE_SLOT_COUNT - 1))
        .unwrap_or(0);
    Some(aux_auto_map([
        Some(runner.turn_press_slot(
            format!("{prefix}.sample.selectedSlot"),
            "Slot",
            NativeMenuAction::PlatformEffect(format!("sample.assign:{index}:{sample_slot}")),
            "Assign",
        )),
        Some(runner.turn_slot(format!("{prefix}.sample.tuneSemis"), "Tune")),
        Some(runner.turn_slot(format!("{prefix}.sample.baseVelocity"), "Base")),
        Some(runner.turn_slot(format!("{prefix}.sample.velocityLevelsEnabled"), "Levels")),
        Some(runner.turn_slot(format!("{prefix}.sample.amp.gainPct"), "Gain")),
    ]))
}

pub(super) fn instrument_mixer_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<AuxAutoMap> {
    if !field.starts_with("mixer.") {
        return None;
    }
    Some(aux_auto_map([
        Some(runner.turn_slot(format!("{prefix}.mixer.volume"), "Vol")),
        Some(runner.turn_slot(format!("{prefix}.mixer.panPos"), "Pan")),
        Some(runner.turn_slot(format!("{prefix}.mixer.route"), "Route")),
    ]))
}
