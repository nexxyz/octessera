use super::*;
use crate::native_menu::NativeMenuValue;
use crate::native_runner::config_document_presence::require_full_runtime_presence;
use std::collections::BTreeSet;

type SavedRange = Option<(i64, i64)>;

/// Each system scalar: saved key, saved and menu range (`None` for switches and choices), and
/// whether an Aux side may turn it.
const SYSTEM_SCALAR_CONTRACT: &[(&str, SavedRange, bool)] = &[
    ("masterVolume", Some((0, 100)), true),
    ("ghostTiles", None, true),
    ("auxAutoHints", None, true),
    ("auxManualHints", None, true),
    ("inputEventsWhilePaused", None, true),
    ("numericDisplayMode", None, true),
    ("mainFont", None, true),
    ("smallFont", None, true),
    ("homeVisual", None, true),
    ("perfViewOnBack", None, true),
    ("perfViewAtBoot", None, true),
    ("dimTimerSeconds", Some((0, 600)), true),
    ("screenSleepSeconds", Some((0, 600)), true),
    ("displayBrightness", Some((1, 16)), true),
    ("gridBrightness", Some((10, 100)), true),
    ("buttonBrightness", Some((10, 100)), true),
    ("autoSaveDefault", None, true),
    ("rollingBackups", None, true),
    ("auxAutoMapEnabled", None, false),
    ("auxPriority", None, false),
];

const PORTABLE_RUNTIME_KEYS: &[&str] = &[
    "activeBehavior",
    "activeLayerIndex",
    "auxBindings",
    "faders",
    "instruments",
    "layers",
    "linkLfos",
    "midiCcMappings",
    "mixer",
    "playFx",
    "playMode",
    "randomSeed",
    "shiftAuxBindings",
    "sound",
    "transport",
    "xy",
    "xyRelease",
];

fn default_payload() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap()
}

#[test]
fn system_scalars_keep_their_saved_keys_ranges_and_aux_turns() {
    let payload = default_payload();
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (key, range, aux_turn) in SYSTEM_SCALAR_CONTRACT {
        let mut missing = payload["runtimeConfig"].clone();
        missing.as_object_mut().unwrap().remove(*key);
        assert_eq!(
            require_full_runtime_presence(&missing, "runtime").unwrap_err(),
            format!("runtime.{key} is required")
        );
        assert_eq!(supported_aux_turn_key(key), *aux_turn, "{key} Aux turn");
        let Some((min, max)) = range else {
            continue;
        };
        for (value, accepted) in [
            (*min, true),
            (*max, true),
            (min - 1, false),
            (max + 1, false),
        ] {
            let mut edited = payload.clone();
            edited["runtimeConfig"][*key] = json!(value);
            assert_eq!(
                validate_config_payload(&edited).is_ok(),
                accepted,
                "{key} = {value}"
            );
        }
        let item = runner.menu.item_for_key(key).unwrap();
        let NativeMenuValue::Number {
            min: menu_min,
            max: menu_max,
            ..
        } = item.value
        else {
            panic!("{key} is not a number row");
        };
        assert_eq!((i64::from(menu_min), i64::from(menu_max)), (*min, *max));
    }
}

#[test]
fn portable_runtime_keeps_only_the_patch_keys() {
    let payload = default_payload();
    let portable = RuntimeConfigDto::from_value(&payload["runtimeConfig"])
        .unwrap()
        .portable_value()
        .unwrap();
    let keys = portable
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(keys, PORTABLE_RUNTIME_KEYS.iter().copied().collect());
}

#[test]
fn timer_and_brightness_edits_stop_at_the_menu_range() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for key in ["dimTimerSeconds", "screenSleepSeconds", "gridBrightness"] {
        for _ in 0..80 {
            runner.edit_menu_key(key, 1).unwrap();
        }
    }
    let ui = &runner.display.ui;
    assert_eq!(
        (
            ui.dim_timer_seconds,
            ui.screen_sleep_seconds,
            ui.grid_brightness
        ),
        (600, 600, 100)
    );
}

#[test]
fn brightness_selectors_report_the_scalar_table_range_at_both_ends() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    for (delta, display, button) in [(80, 16, 100), (-80, 1, 10)] {
        runner.edit_menu_key("displayBrightness", delta).unwrap();
        runner.edit_menu_key("buttonBrightness", delta).unwrap();
        assert_eq!(runner.menu.selected_display_brightness(), Some(display));
        assert_eq!(runner.menu.selected_button_brightness(), Some(button));
    }
}

#[test]
fn loaded_led_brightness_is_clamped_to_the_scalar_table_range() {
    let mut runtime = default_payload()["runtimeConfig"].clone();
    runtime["gridBrightness"] = json!(3);
    runtime["buttonBrightness"] = json!(0);
    runtime["displayBrightness"] = json!(0);
    let dto = RuntimeConfigDto::from_value(&runtime).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_display_payload(&dto);
    let ui = &runner.display.ui;
    assert_eq!(
        (
            ui.grid_brightness,
            ui.button_brightness,
            ui.display_brightness
        ),
        (10, 10, 1)
    );
}

#[test]
fn only_table_scalars_are_turnable_brightness_and_screen_keys() {
    for key in ["gridBrightness", "buttonBrightness", "displayBrightness"] {
        assert!(supported_aux_turn_key(key), "{key}");
    }
    for key in ["screenSleepSeconds", "dimTimerSeconds"] {
        assert!(supported_aux_turn_key(key), "{key}");
    }
    for key in ["ledBrightness", "screenFlash", "screen", "Brightness"] {
        assert!(!supported_aux_turn_key(key), "{key}");
    }
}
