use super::super::{
    cutoff_display_to_hz, cutoff_hz_to_display, drum_config, set_json_path_number, value_i32_at,
    NativeRunner,
};
use crate::protocol::RuntimeAudioCommand;
use serde_json::{json, Value};

pub(super) fn apply_drum_menu_key(
    runner: &mut NativeRunner,
    slot: usize,
    key: &str,
    field: &str,
) -> Option<bool> {
    if field == "drum.voice" {
        return apply_selected_drum_voice(runner, slot, key);
    }
    if let Some(voice_path) = field.strip_prefix("drum.voices.") {
        let (voice, param) = voice_path.split_once('.')?;
        let voice = voice.parse::<usize>().ok()?;
        if param == "sound" {
            return apply_drum_voice_sound(runner, slot, key, voice);
        }
        return apply_drum_voice_numeric(runner, slot, key, field);
    }
    if field == "drum.filter.type" {
        return apply_drum_filter_type(runner, slot, key);
    }
    apply_drum_common_numeric(runner, slot, key, field)
}

fn apply_selected_drum_voice(runner: &mut NativeRunner, slot: usize, key: &str) -> Option<bool> {
    let selected = runner
        .menu
        .value_for_key(key)?
        .strip_prefix('V')?
        .split_once(':')?
        .0
        .parse::<usize>()
        .ok()?
        .checked_sub(1)?;
    if selected >= 8 {
        return None;
    }
    let current = runner.drum_selected_voices.get_mut(slot)?;
    if *current != selected {
        *current = selected;
        runner.rematerialize_menu_around_key(key);
    }
    Some(true)
}

fn apply_drum_voice_sound(
    runner: &mut NativeRunner,
    slot: usize,
    key: &str,
    voice: usize,
) -> Option<bool> {
    let sound = runner.menu.value_for_key(key)?;
    let next = drum_config::drum_voice_default(&sound)?;
    let voices = runner
        .instruments
        .get_mut(slot)?
        .drum_config
        .get_mut("voices")?
        .as_array_mut()?;
    let current = voices.get_mut(voice)?;
    if current["sound"] != sound {
        *current = next;
        if let Some(config) = runner.instrument_audio_config(slot) {
            runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: slot,
                generation: 0,
                config,
            });
        }
        runner.rematerialize_menu_around_key(key);
        runner.mark_fast_autosave_dirty();
    }
    Some(true)
}

fn apply_drum_voice_numeric(
    runner: &mut NativeRunner,
    slot: usize,
    key: &str,
    field: &str,
) -> Option<bool> {
    let (voice, param, min, max) = drum_config::voice_numeric_field(field)?;
    let value = runner.menu.number_for_key(key)?.clamp(min, max);
    let current = runner
        .instruments
        .get_mut(slot)?
        .drum_config
        .get_mut("voices")?
        .as_array_mut()?
        .get_mut(voice)?;
    if current.get(param).and_then(Value::as_i64) != Some(i64::from(value)) {
        current.as_object_mut()?.insert(param.into(), json!(value));
        if !runner.rebase_and_recompose_modulation_key(key) {
            runner.queue_audio_command(RuntimeAudioCommand::SetDrumParam {
                instrument_slot: slot,
                voice: voice as u8,
                generation: 0,
                path: format!("drum.{param}"),
                value: value as f32,
            });
        }
        runner.mark_fast_autosave_dirty();
    }
    Some(true)
}

fn apply_drum_filter_type(runner: &mut NativeRunner, slot: usize, key: &str) -> Option<bool> {
    let value = runner.menu.value_for_key(key)?;
    let instrument = runner.instruments.get_mut(slot)?;
    if instrument.drum_config["filter"]["type"] != value {
        instrument.drum_config["filter"]["type"] = json!(value);
        if let Some(config) = runner.instrument_audio_config(slot) {
            runner.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot: slot,
                generation: 0,
                config,
            });
        }
        runner.mark_fast_autosave_dirty();
    }
    Some(true)
}

fn apply_drum_common_numeric(
    runner: &mut NativeRunner,
    slot: usize,
    key: &str,
    field: &str,
) -> Option<bool> {
    let (path, min, max) = drum_config::common_numeric_field(field)?;
    let value = runner.menu.number_for_key(key)?.clamp(min, max);
    let instrument = runner.instruments.get_mut(slot)?;
    let current = value_i32_at(&instrument.drum_config, path, i32::MIN);
    let unchanged = if field == "drum.filter.cutoffHz" {
        cutoff_hz_to_display(current) == value
    } else {
        current == value
    };
    if !unchanged {
        let stored = if field == "drum.filter.cutoffHz" {
            cutoff_display_to_hz(value)
        } else {
            value
        };
        set_json_path_number(&mut instrument.drum_config, path, f64::from(stored));
        if !runner.rebase_and_recompose_modulation_key(key) {
            runner.queue_audio_command(RuntimeAudioCommand::SetDrumParam {
                instrument_slot: slot,
                voice: 0,
                generation: 0,
                path: field.into(),
                value: stored as f32,
            });
        }
        runner.mark_fast_autosave_dirty();
    }
    Some(true)
}
