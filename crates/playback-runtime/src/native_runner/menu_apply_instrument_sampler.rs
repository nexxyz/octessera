use super::menu_apply_fast::value_changed;
use super::{
    set_json_path_number, set_json_path_string, value_i32_at, value_string_at,
    NativeInstrumentSlot, NativeRunner,
};
use crate::native_menu::NativeMenuModel;

impl NativeRunner {
    pub(super) fn apply_sampler_menu_key(&mut self, index: usize, key: &str) -> bool {
        let Some(field) = key.strip_prefix(&format!("instruments.{index}.sample.")) else {
            return false;
        };
        let Some(instrument) = self.instruments.get_mut(index) else {
            return false;
        };
        apply_sampler_menu_field(&self.menu, key, field, instrument)
    }
}

fn apply_sampler_menu_field(
    menu: &NativeMenuModel,
    key: &str,
    field: &str,
    instrument: &mut NativeInstrumentSlot,
) -> bool {
    let level = |value: i32| value.clamp(1, 127) as u8;
    match field {
        "selectedSlot" => menu
            .value_for_key(key)
            .and_then(|value| value.parse::<usize>().ok())
            .and_then(|value| value.checked_sub(1))
            .is_some_and(|slot| value_changed(&mut instrument.selected_sample_slot, slot.min(7))),
        "baseVelocity" => menu
            .number_for_key(key)
            .is_some_and(|value| value_changed(&mut instrument.sample_base_velocity, level(value))),
        "velocityLevelsEnabled" => menu.value_for_key(key).is_some_and(|value| {
            value_changed(
                &mut instrument.sample_velocity_levels_enabled,
                value == "true",
            )
        }),
        "velocityLevels.high" => menu
            .number_for_key(key)
            .is_some_and(|value| value_changed(&mut instrument.sample_velocity_high, level(value))),
        "velocityLevels.medium" => menu.number_for_key(key).is_some_and(|value| {
            value_changed(&mut instrument.sample_velocity_medium, level(value))
        }),
        "velocityLevels.low" => menu
            .number_for_key(key)
            .is_some_and(|value| value_changed(&mut instrument.sample_velocity_low, level(value))),
        "filter.type" => menu.value_for_key(key).is_some_and(|filter_type| {
            if value_string_at(&instrument.sample_filter, &["type"], "lowpass") == filter_type {
                return false;
            }
            set_json_path_string(&mut instrument.sample_filter, &["type"], &filter_type);
            true
        }),
        "filter.envAmountPct" => set_sampler_number(
            menu,
            key,
            &mut instrument.sample_filter,
            "envAmountPct",
            -100,
            100,
        ),
        "filter.keyTrackingPct" => set_sampler_number(
            menu,
            key,
            &mut instrument.sample_filter,
            "keyTrackingPct",
            0,
            100,
        ),
        _ => {
            let Some((env, param)) = field.split_once('.') else {
                return false;
            };
            let config = match env {
                "ampEnv" => &mut instrument.sample_amp_env,
                "filterEnv" => &mut instrument.sample_filter_env,
                _ => return false,
            };
            let max = match param {
                "attackMs" | "decayMs" => 5000,
                "sustainPct" => 100,
                "releaseMs" => 10000,
                _ => return false,
            };
            set_sampler_number(menu, key, config, param, 0, max)
        }
    }
}

fn set_sampler_number(
    menu: &NativeMenuModel,
    key: &str,
    config: &mut serde_json::Value,
    field: &str,
    min: i32,
    max: i32,
) -> bool {
    let Some(value) = menu.number_for_key(key).map(|value| value.clamp(min, max)) else {
        return false;
    };
    if value_i32_at(config, &[field], i32::MIN) == value {
        return false;
    }
    set_json_path_number(config, &[field], f64::from(value));
    true
}
