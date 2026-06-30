use super::menu_apply_instrument_midi::apply_midi_menu_fields;
use super::menu_apply_instrument_synth::apply_synth_menu_fields;
use super::{
    cutoff_display_to_hz, cutoff_hz_to_display, derive_instrument_name, set_json_path_number,
    set_json_path_string, value_i32_at, value_string_at, NativeInstrumentSlot, NativeRunner,
};
use crate::native_menu::NativeMenuModel;
use platform_core::PAN_POSITION_COUNT;

impl NativeRunner {
    pub(super) fn apply_instrument_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.instruments.len() {
            let Some(instrument) = self.instruments.get_mut(index) else {
                continue;
            };
            changed |= apply_identity_menu_fields(&self.menu, index, instrument);
            changed |= apply_mixer_menu_fields(&self.menu, index, instrument);
            changed |= apply_synth_menu_fields(&self.menu, index, instrument);
            changed |= apply_sampler_menu_fields(&self.menu, index, instrument);
            changed |= apply_midi_menu_fields(&self.menu, index, instrument);
        }
        changed
    }
}

fn apply_identity_menu_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    let before_name = instrument.name.clone();
    if let Some(kind) = menu.value_for_key(&format!("instruments.{index}.type")) {
        if instrument.kind != kind {
            instrument.kind = kind;
            if instrument.auto_name {
                instrument.name = derive_instrument_name(index, &instrument.kind);
            }
            changed = true;
        }
    }
    if let Some(note_behavior) = menu.value_for_key(&format!("instruments.{index}.noteBehavior")) {
        if instrument.note_behavior != note_behavior {
            instrument.note_behavior = note_behavior;
            changed = true;
        }
    }
    if let Some(auto_name) = menu
        .value_for_key(&format!("instruments.{index}.autoName"))
        .map(|value| value == "true")
    {
        if instrument.auto_name != auto_name {
            instrument.auto_name = auto_name;
            if auto_name {
                instrument.name = derive_instrument_name(index, &instrument.kind);
            }
            changed = true;
        }
    }
    let name_key = format!("instruments.{index}.name");
    if menu.current_key() == Some(name_key.as_str()) {
        if let Some(name) = menu.value_for_key(&name_key) {
            if name != before_name {
                instrument.name = name;
                instrument.auto_name = false;
                changed = true;
            }
        }
    }
    if instrument.auto_name {
        let derived = derive_instrument_name(index, &instrument.kind);
        if instrument.name != derived {
            instrument.name = derived;
            changed = true;
        }
    }
    changed
}

fn apply_mixer_menu_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(volume) = menu.number_for_key(&format!("instruments.{index}.mixer.volume")) {
        let volume = volume.clamp(0, 100) as u8;
        if instrument.volume != volume {
            instrument.volume = volume;
            changed = true;
        }
    }
    if let Some(pan_pos) = menu.number_for_key(&format!("instruments.{index}.mixer.panPos")) {
        let pan_pos = pan_pos.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
        if instrument.pan_pos != pan_pos {
            instrument.pan_pos = pan_pos;
            changed = true;
        }
    }
    if let Some(route) = menu.value_for_key(&format!("instruments.{index}.mixer.route")) {
        if instrument.route != route {
            instrument.route = route;
            changed = true;
        }
    }
    changed
}

fn apply_sampler_menu_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(sample_slot) = menu
        .value_for_key(&format!("instruments.{index}.sample.selectedSlot"))
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        let sample_slot = sample_slot.min(7);
        if instrument.selected_sample_slot != sample_slot {
            instrument.selected_sample_slot = sample_slot;
            changed = true;
        }
    }
    changed |= apply_sampler_scalar_fields(menu, index, instrument);
    changed |= apply_sampler_velocity_level_fields(menu, index, instrument);
    changed |= apply_sampler_filter_fields(menu, index, instrument);
    changed |= apply_sampler_env_fields(menu, index, instrument);
    changed
}

fn apply_sampler_scalar_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(tune) = menu.number_for_key(&format!("instruments.{index}.sample.tuneSemis")) {
        let tune = tune.clamp(-24, 24) as i8;
        if instrument.sample_tune_semis != tune {
            instrument.sample_tune_semis = tune;
            changed = true;
        }
    }
    if let Some(gain) = menu.number_for_key(&format!("instruments.{index}.sample.amp.gainPct")) {
        let gain = gain.clamp(0, 100) as u8;
        if instrument.sample_gain_pct != gain {
            instrument.sample_gain_pct = gain;
            changed = true;
        }
    }
    if let Some(base_velocity) =
        menu.number_for_key(&format!("instruments.{index}.sample.baseVelocity"))
    {
        let base_velocity = base_velocity.clamp(1, 127) as u8;
        if instrument.sample_base_velocity != base_velocity {
            instrument.sample_base_velocity = base_velocity;
            changed = true;
        }
    }
    if let Some(velocity_sens) = menu.number_for_key(&format!(
        "instruments.{index}.sample.amp.velocitySensitivityPct"
    )) {
        let velocity_sens = velocity_sens.clamp(0, 100) as u8;
        if instrument.sample_amp_velocity_sensitivity_pct != velocity_sens {
            instrument.sample_amp_velocity_sensitivity_pct = velocity_sens;
            changed = true;
        }
    }
    if let Some(enabled) = menu
        .value_for_key(&format!("instruments.{index}.sample.velocityLevelsEnabled"))
        .map(|value| value == "true")
    {
        if instrument.sample_velocity_levels_enabled != enabled {
            instrument.sample_velocity_levels_enabled = enabled;
            changed = true;
        }
    }
    changed
}

fn apply_sampler_velocity_level_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    for (suffix, current) in [
        ("velocityLevels.high", &mut instrument.sample_velocity_high),
        (
            "velocityLevels.medium",
            &mut instrument.sample_velocity_medium,
        ),
        ("velocityLevels.low", &mut instrument.sample_velocity_low),
    ] {
        if let Some(value) = menu.number_for_key(&format!("instruments.{index}.sample.{suffix}")) {
            let value = value.clamp(1, 127) as u8;
            if *current != value {
                *current = value;
                changed = true;
            }
        }
    }
    changed
}

fn apply_sampler_filter_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    if let Some(filter_type) =
        menu.value_for_key(&format!("instruments.{index}.sample.filter.type"))
    {
        if value_string_at(&instrument.sample_filter, &["type"], "lowpass") != filter_type {
            set_json_path_string(&mut instrument.sample_filter, &["type"], &filter_type);
            changed = true;
        }
    }
    for (suffix, path, min, max) in sampler_filter_field_specs() {
        if let Some(value) = menu.number_for_key(&format!("instruments.{index}.sample.{suffix}")) {
            let value = value.clamp(*min, *max);
            let stored_value = if *suffix == "filter.cutoffHz" {
                cutoff_display_to_hz(value)
            } else {
                value
            };
            let current = value_i32_at(&instrument.sample_filter, path, i32::MIN);
            let unchanged = if *suffix == "filter.cutoffHz" {
                cutoff_hz_to_display(current) == value
            } else {
                current == stored_value
            };
            if !unchanged {
                set_json_path_number(&mut instrument.sample_filter, path, f64::from(stored_value));
                changed = true;
            }
        }
    }
    changed
}

fn sampler_filter_field_specs() -> &'static [(&'static str, &'static [&'static str], i32, i32)] {
    &[
        ("filter.cutoffHz", &["cutoffHz"], 0, 255),
        ("filter.resonance", &["resonance"], 0, 255),
        ("filter.envAmountPct", &["envAmountPct"], -100, 100),
        ("filter.keyTrackingPct", &["keyTrackingPct"], 0, 100),
    ]
}

fn apply_sampler_env_fields(
    menu: &NativeMenuModel,
    index: usize,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let mut changed = false;
    for (prefix_key, target, field, min, max) in sampler_env_field_specs() {
        if let Some(value) =
            menu.number_for_key(&format!("instruments.{index}.sample.{prefix_key}.{field}"))
        {
            let value = value.clamp(*min, *max);
            let config = if *target == "amp" {
                &mut instrument.sample_amp_env
            } else {
                &mut instrument.sample_filter_env
            };
            if value_i32_at(config, &[*field], i32::MIN) != value {
                set_json_path_number(config, &[*field], f64::from(value));
                changed = true;
            }
        }
    }
    changed
}

fn sampler_env_field_specs() -> &'static [(&'static str, &'static str, &'static str, i32, i32)] {
    &[
        ("ampEnv", "amp", "attackMs", 0, 5000),
        ("ampEnv", "amp", "decayMs", 0, 5000),
        ("ampEnv", "amp", "sustainPct", 0, 100),
        ("ampEnv", "amp", "releaseMs", 0, 10000),
        ("filterEnv", "filter", "attackMs", 0, 5000),
        ("filterEnv", "filter", "decayMs", 0, 5000),
        ("filterEnv", "filter", "sustainPct", 0, 100),
        ("filterEnv", "filter", "releaseMs", 0, 10000),
    ]
}
