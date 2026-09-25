use super::super::{
    cutoff_display_to_hz, cutoff_hz_to_display, set_json_path_number, set_json_path_string,
    value_i32_at, value_string_at, NativeRunner,
};
use crate::protocol::RuntimeAudioCommand;

pub(super) fn apply_pluck_menu_key(
    runner: &mut NativeRunner,
    index: usize,
    key: &str,
    suffix: &str,
) -> Option<bool> {
    if suffix == "pluck.filter.type" {
        let value = runner.menu.value_for_key(key)?;
        let instrument = runner.instruments.get_mut(index)?;
        if value_string_at(&instrument.pluck_config, &["filter", "type"], "") != value {
            set_json_path_string(&mut instrument.pluck_config, &["filter", "type"], &value);
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
    let field = suffix.strip_prefix("pluck.")?;
    let (path, min, max) = numeric_field(field)?;
    let value = runner.menu.number_for_key(key)?.clamp(min, max);
    let instrument = runner.instruments.get_mut(index)?;
    let current = value_i32_at(&instrument.pluck_config, path, i32::MIN);
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
        set_json_path_number(&mut instrument.pluck_config, path, f64::from(stored));
        if !runner.rebase_and_recompose_modulation_key(key) {
            runner.queue_audio_command(RuntimeAudioCommand::SetPluckParam {
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
    let (path, range): (&[&str], (i32, i32)) = match field {
        "decayMs" => (&["decayMs"], (100, 5000)),
        "brightnessPct" => (&["brightnessPct"], (0, 100)),
        "pickPositionPct" => (&["pickPositionPct"], (5, 50)),
        "amp.gainPct" => (&["amp", "gainPct"], (0, 100)),
        "amp.velocitySensitivityPct" => (&["amp", "velocitySensitivityPct"], (0, 100)),
        "filter.cutoffHz" => (&["filter", "cutoffHz"], (0, 255)),
        "filter.resonance" => (&["filter", "resonance"], (0, 255)),
        "filter.envAmountPct" => (&["filter", "envAmountPct"], (-100, 100)),
        "filter.keyTrackingPct" => (&["filter", "keyTrackingPct"], (0, 100)),
        "ampEnv.attackMs" => (&["ampEnv", "attackMs"], (0, 5000)),
        "ampEnv.decayMs" => (&["ampEnv", "decayMs"], (0, 5000)),
        "ampEnv.sustainPct" => (&["ampEnv", "sustainPct"], (0, 100)),
        "ampEnv.releaseMs" => (&["ampEnv", "releaseMs"], (0, 10000)),
        "filterEnv.attackMs" => (&["filterEnv", "attackMs"], (0, 5000)),
        "filterEnv.decayMs" => (&["filterEnv", "decayMs"], (0, 5000)),
        "filterEnv.sustainPct" => (&["filterEnv", "sustainPct"], (0, 100)),
        "filterEnv.releaseMs" => (&["filterEnv", "releaseMs"], (0, 10000)),
        _ => return None,
    };
    Some((path, range.0, range.1))
}
