use super::*;
use crate::native_runner::aux_auto_map::ResolvedAuxSlot;

pub(super) fn instrument_filter_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
    if field.starts_with("synth.filter.") {
        Some([
            Some(runner.turn_slot(format!("{prefix}.synth.filter.cutoffHz"), "Cutoff")),
            Some(runner.turn_slot(format!("{prefix}.synth.filter.resonance"), "Res")),
            Some(runner.turn_slot(format!("{prefix}.synth.filter.envAmountPct"), "Env")),
            Some(runner.turn_slot(format!("{prefix}.synth.filter.keyTrackingPct"), "Key")),
        ])
    } else if field.starts_with("sample.filter.") {
        Some([
            Some(runner.turn_slot(format!("{prefix}.sample.filter.cutoffHz"), "Cutoff")),
            Some(runner.turn_slot(format!("{prefix}.sample.filter.resonance"), "Res")),
            Some(runner.turn_slot(format!("{prefix}.sample.filter.envAmountPct"), "Env")),
            Some(runner.turn_slot(format!("{prefix}.sample.filter.keyTrackingPct"), "Key")),
        ])
    } else {
        None
    }
}

pub(super) fn instrument_envelope_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
    if field.starts_with("synth.ampEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.synth.ampEnv")))
    } else if field.starts_with("synth.filterEnv.") {
        Some(runner.env_auto_map(&format!("{prefix}.synth.filterEnv")))
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
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
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
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
    if field.starts_with("synth.amp.") {
        Some([
            Some(runner.turn_slot(format!("{prefix}.synth.amp.gainPct"), "Gain")),
            Some(runner.turn_slot(format!("{prefix}.synth.amp.velocitySensitivityPct"), "Vel")),
            None,
            None,
        ])
    } else if field.starts_with("sample.amp.") {
        Some([
            Some(runner.turn_slot(format!("{prefix}.sample.amp.gainPct"), "Gain")),
            Some(runner.turn_slot(format!("{prefix}.sample.amp.velocitySensitivityPct"), "Vel")),
            None,
            None,
        ])
    } else {
        None
    }
}

pub(super) fn instrument_sample_auto_map(
    runner: &NativeRunner,
    index: usize,
    field: &str,
    prefix: &str,
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
    if !field.starts_with("sample.") {
        return None;
    }
    let sample_slot = runner
        .instruments
        .get(index)
        .map(|instrument| instrument.selected_sample_slot.min(SAMPLE_SLOT_COUNT - 1))
        .unwrap_or(0);
    Some([
        Some(runner.turn_press_slot(
            format!("{prefix}.sample.selectedSlot"),
            "Slot",
            NativeMenuAction::PlatformEffect(format!("sample.assign:{index}:{sample_slot}")),
            "Assign",
        )),
        Some(runner.turn_slot(format!("{prefix}.sample.baseVelocity"), "Base")),
        Some(runner.turn_slot(format!("{prefix}.sample.tuneSemis"), "Tune")),
        Some(runner.turn_slot(format!("{prefix}.sample.velocityLevelsEnabled"), "Levels")),
    ])
}

pub(super) fn instrument_mixer_auto_map(
    runner: &NativeRunner,
    field: &str,
    prefix: &str,
) -> Option<[Option<ResolvedAuxSlot>; 4]> {
    if !field.starts_with("mixer.") {
        return None;
    }
    Some([
        Some(runner.turn_slot(format!("{prefix}.mixer.volume"), "Vol")),
        Some(runner.turn_slot(format!("{prefix}.mixer.panPos"), "Pan")),
        Some(runner.turn_slot(format!("{prefix}.mixer.route"), "Route")),
        None,
    ])
}
