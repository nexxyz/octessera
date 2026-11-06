use super::*;
use crate::instrument_presets::instrument_presets;
use crate::native_menu::{NativeMenuAction, NativeMenuValue};

const FAMILIES: [(&str, &str, usize); 4] = [
    ("synth", "synth.preset", 11),
    ("fm", "fm.preset", 11),
    ("pluck", "pluck.preset", 11),
    ("drum", "drum.kit", 10),
];

#[test]
fn factory_preset_names_are_short_title_case_and_unique() {
    for (family, _, count) in FAMILIES {
        let presets = instrument_presets(family);
        assert_eq!(presets.len(), count, "{family}");
        assert_eq!(
            presets[0].set,
            json!({}),
            "{family}: the first preset is the default"
        );
        let mut ids = std::collections::BTreeSet::new();
        let mut labels = std::collections::BTreeSet::new();
        for preset in presets {
            let (id, label) = (preset.id.as_str(), preset.label.as_str());
            assert!(id.len() <= 12, "{family} {id}: id too long");
            assert!(
                id.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_'),
                "{family} {id}: id is not snake_case"
            );
            assert!(
                label.chars().count() <= 12,
                "{family} {label}: label too long"
            );
            assert!(
                label
                    .split([' ', '-'])
                    .all(|word| word.starts_with(|ch: char| ch.is_ascii_uppercase())),
                "{family} {label}: label is not Title Case"
            );
            assert!(ids.insert(id), "{family} {id}: duplicate id");
            assert!(labels.insert(label), "{family} {label}: duplicate label");
        }
    }
}

fn leaves(value: &Value, path: String, out: &mut Vec<(String, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                leaves(child, child_path, out);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                leaves(child, format!("{path}.{index}"), out);
            }
        }
        _ => out.push((path, value.clone())),
    }
}

fn assert_in_menu_range(runner: &NativeRunner, family: &str, field: &str, value: &Value) {
    let context = format!("{family} {field} = {value}");
    if field.ends_with("cutoffHz") {
        let hz = value.as_i64().unwrap();
        assert!((20..=20_000).contains(&hz), "{context}");
        return;
    }
    if let Some((_, _, min, max)) =
        super::super::drum_config::voice_numeric_field(&format!("drum.{field}"))
    {
        let number = value.as_i64().unwrap();
        assert!(
            (i64::from(min)..=i64::from(max)).contains(&number),
            "{context}"
        );
        return;
    }
    let key = format!("instruments.0.{family}.{field}");
    let item = runner
        .menu
        .item_for_key(&key)
        .unwrap_or_else(|| panic!("{context}: no menu row {key}"));
    match item.value {
        NativeMenuValue::Number { min, max, .. } => {
            let number = value.as_i64().unwrap();
            assert!(
                (i64::from(min)..=i64::from(max)).contains(&number),
                "{context}"
            );
        }
        NativeMenuValue::Enum { .. } => {
            assert_eq!(
                runner.menu.value_for_key(&key).as_deref(),
                value.as_str(),
                "{context}"
            );
        }
        other => panic!("{context}: unexpected menu row {other:?}"),
    }
}

#[test]
fn every_factory_preset_override_is_inside_its_parameter_range() {
    for (family, action, _) in FAMILIES {
        for preset in instrument_presets(family) {
            let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
            runner
                .execute_confirmed_action(NativeMenuAction::PlatformEffect(format!(
                    "{action}:0:{}",
                    preset.id
                )))
                .unwrap();
            assert_eq!(runner.instruments[0].kind, family, "{family} {}", preset.id);
            validate_config_payload(&runner.config_payload())
                .unwrap_or_else(|error| panic!("{family} {}: {error}", preset.id));
            runner.menu.rebuild(runner.menu_config());
            let mut overrides = Vec::new();
            leaves(&preset.set, String::new(), &mut overrides);
            for (field, value) in overrides {
                assert_in_menu_range(&runner, family, &field, &value);
            }
        }
    }
}
