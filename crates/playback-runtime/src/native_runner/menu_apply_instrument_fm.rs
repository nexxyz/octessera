use super::super::{
    cutoff_display_to_hz, cutoff_hz_to_display, set_json_path_number, set_json_path_string,
    value_i32_at, value_string_at, NativeRunner,
};
use crate::protocol::RuntimeAudioCommand;

pub(super) fn apply_fm_menu_key(
    runner: &mut NativeRunner,
    index: usize,
    key: &str,
    suffix: &str,
) -> Option<bool> {
    if matches!(suffix, "fm.ratio" | "fm.filter.type") {
        let value = runner.menu.value_for_key(key)?;
        let instrument = runner.instruments.get_mut(index)?;
        let path = if suffix == "fm.ratio" {
            &["ratio"][..]
        } else {
            &["filter", "type"][..]
        };
        if value_string_at(&instrument.fm_config, path, "") != value {
            set_json_path_string(&mut instrument.fm_config, path, &value);
            if let Some(config) = runner.instrument_audio_config(index) {
                runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                    instrument_slot: index,
                    generation: 0,
                    config,
                });
            }
            runner.mark_fast_autosave_dirty();
        }
        return Some(true);
    }
    let field = suffix.strip_prefix("fm.")?;
    let (path, min, max) = numeric_field(field)?;
    let value = runner.menu.number_for_key(key)?.clamp(min, max);
    let instrument = runner.instruments.get_mut(index)?;
    let current = value_i32_at(&instrument.fm_config, path, i32::MIN);
    let unchanged = if field == "filter.cutoffHz" {
        cutoff_hz_to_display(current) == value
    } else {
        current == value
    };
    if !unchanged {
        let stored = if field == "filter.cutoffHz" {
            cutoff_display_to_hz(value)
        } else {
            value
        };
        set_json_path_number(&mut instrument.fm_config, path, f64::from(stored));
        if !runner.rebase_and_recompose_modulation_key(key) {
            runner.queue_audio_command(RuntimeAudioCommand::SetFmParam {
                instrument_slot: index,
                generation: 0,
                path: suffix.into(),
                value: stored as f32,
            });
        }
        runner.mark_fast_autosave_dirty();
    }
    Some(true)
}

pub(in super::super) fn numeric_field(field: &str) -> Option<(&[&str], i32, i32)> {
    let range = match field {
        "index" | "amp.gainPct" | "amp.velocitySensitivityPct" | "filter.keyTrackingPct" => {
            (0, 100)
        }
        "filter.cutoffHz" | "filter.resonance" => (0, 255),
        "filter.envAmountPct" => (-100, 100),
        "indexEnv.attackMs" | "indexEnv.decayMs" | "ampEnv.attackMs" | "ampEnv.decayMs"
        | "filterEnv.attackMs" | "filterEnv.decayMs" => (0, 5000),
        "indexEnv.releaseMs" | "ampEnv.releaseMs" | "filterEnv.releaseMs" => (0, 10000),
        "indexEnv.sustainPct" | "ampEnv.sustainPct" | "filterEnv.sustainPct" => (0, 100),
        _ => return None,
    };
    let path = match field.split_once('.') {
        Some(("indexEnv", "attackMs")) => &["indexEnv", "attackMs"][..],
        Some(("indexEnv", "decayMs")) => &["indexEnv", "decayMs"][..],
        Some(("indexEnv", "sustainPct")) => &["indexEnv", "sustainPct"][..],
        Some(("indexEnv", "releaseMs")) => &["indexEnv", "releaseMs"][..],
        Some(("amp", "gainPct")) => &["amp", "gainPct"][..],
        Some(("amp", "velocitySensitivityPct")) => &["amp", "velocitySensitivityPct"][..],
        Some(("filter", "cutoffHz")) => &["filter", "cutoffHz"][..],
        Some(("filter", "resonance")) => &["filter", "resonance"][..],
        Some(("filter", "envAmountPct")) => &["filter", "envAmountPct"][..],
        Some(("filter", "keyTrackingPct")) => &["filter", "keyTrackingPct"][..],
        Some(("ampEnv", "attackMs")) => &["ampEnv", "attackMs"][..],
        Some(("ampEnv", "decayMs")) => &["ampEnv", "decayMs"][..],
        Some(("ampEnv", "sustainPct")) => &["ampEnv", "sustainPct"][..],
        Some(("ampEnv", "releaseMs")) => &["ampEnv", "releaseMs"][..],
        Some(("filterEnv", "attackMs")) => &["filterEnv", "attackMs"][..],
        Some(("filterEnv", "decayMs")) => &["filterEnv", "decayMs"][..],
        Some(("filterEnv", "sustainPct")) => &["filterEnv", "sustainPct"][..],
        Some(("filterEnv", "releaseMs")) => &["filterEnv", "releaseMs"][..],
        None => &["index"][..],
        _ => return None,
    };
    Some((path, range.0, range.1))
}
