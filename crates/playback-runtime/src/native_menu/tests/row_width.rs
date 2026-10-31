use super::*;
use crate::native_menu::format::{format_item_bar_value, format_item_row};
use crate::native_runner::{NativeRunner, NativeRunnerConfig};
use crate::oled_frame::{
    menu_row_clips, menu_row_label_overflows, OledMenuRowInput, OledMenuRowKind,
};

const NUMBER_SAMPLES: i32 = 2000;

fn shown_values(item: &NativeMenuItem) -> Vec<NativeMenuValue> {
    match &item.value {
        NativeMenuValue::Enum { options, .. } => (0..options.len())
            .map(|selected| NativeMenuValue::Enum {
                options: options.clone(),
                selected,
            })
            .collect(),
        NativeMenuValue::Number {
            min,
            max,
            step,
            curve,
            ..
        } => {
            let stride = ((max - min) / NUMBER_SAMPLES).max(*step).max(1) as usize;
            (*min..=*max)
                .step_by(stride)
                .chain([*max])
                .map(|value| NativeMenuValue::Number {
                    value,
                    min: *min,
                    max: *max,
                    step: *step,
                    curve: *curve,
                })
                .collect()
        }
        NativeMenuValue::Bool { .. } => [false, true]
            .map(|value| NativeMenuValue::Bool { value })
            .to_vec(),
        other => vec![other.clone()],
    }
}

fn oled_row(item: &NativeMenuItem) -> Option<OledMenuRowInput> {
    let row = format_item_row(item, false, false).1?;
    Some(OledMenuRowInput {
        kind: match row.kind {
            NativeMenuRowKind::Value => OledMenuRowKind::Value,
            NativeMenuRowKind::Group => OledMenuRowKind::Group,
            NativeMenuRowKind::Action => OledMenuRowKind::Action,
            NativeMenuRowKind::Entry => OledMenuRowKind::Entry,
        },
        label: row.label,
        value: row.value,
        text_cursor: None,
        aux_cap: None,
        preview: false,
    })
}

fn shows_a_name(item: &NativeMenuItem) -> bool {
    item.key
        .as_deref()
        .is_some_and(|key| key.ends_with("behaviorId"))
        || item.label.split_once(": ").is_some_and(|(scope, _)| {
            scope.len() > 1
                && scope.starts_with(['L', 'I', 'B'])
                && scope[1..].bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn first_clipped_row(item: &NativeMenuItem) -> Option<String> {
    if shows_a_name(item) {
        return None;
    }
    for value in shown_values(item) {
        let shown = NativeMenuItem {
            value,
            ..item.clone()
        };
        let Some(mut row) = oled_row(&shown) else {
            continue;
        };
        if row.kind == OledMenuRowKind::Entry {
            continue;
        }
        if matches!(item.value, NativeMenuValue::Text { .. }) {
            row.value = "W".repeat(40);
            if menu_row_label_overflows(&row, false) {
                return Some(format!("{} | <name>", row.label));
            }
            continue;
        }
        let bar = format_item_bar_value(&shown, "bar").is_some();
        if menu_row_clips(&row, false) || bar && menu_row_clips(&row, true) {
            return Some(format!("{} | {}", row.label, row.value));
        }
    }
    None
}

fn row_shape(item: &NativeMenuItem) -> String {
    let range = match &item.value {
        NativeMenuValue::Enum { options, .. } => options.join(","),
        NativeMenuValue::Number { min, max, step, .. } => format!("{min}..{max}/{step}"),
        NativeMenuValue::Group | NativeMenuValue::Action(_) => item.children.is_empty().to_string(),
        NativeMenuValue::Bool { .. } => "bool".into(),
        NativeMenuValue::Text { .. } => "text".into(),
    };
    format!("{}|{:?}|{range}", item.label, item.key)
}

fn collect_clipped(
    item: &NativeMenuItem,
    path: &str,
    seen: &mut std::collections::HashSet<String>,
    clipped: &mut std::collections::BTreeMap<String, String>,
) {
    let path = format!("{path} > {}", item.label);
    if let Some(row) = seen
        .insert(row_shape(item))
        .then(|| first_clipped_row(item))
        .flatten()
    {
        clipped
            .entry(row)
            .or_insert_with(|| format!("{path} [{}]", item.key.as_deref().unwrap_or("-")));
    }
    for child in &item.children {
        collect_clipped(child, &path, seen, clipped);
    }
}

fn behavior_configs() -> Vec<NativeMenuConfig> {
    platform_core::list_native_behavior_ids()
        .iter()
        .map(|behavior_id| {
            NativeRunner::new(NativeRunnerConfig {
                behavior_id: (*behavior_id).into(),
                ..NativeRunnerConfig::default()
            })
            .unwrap()
            .menu_config()
        })
        .collect()
}

fn system_configs() -> Vec<NativeMenuConfig> {
    [
        crate::native_runner::UsbDataRole::Gadget,
        crate::native_runner::UsbDataRole::Host,
    ]
    .map(|role| {
        let mut cfg = config();
        cfg.board = true;
        cfg.hdmi_mode = "cycle-behaviors".into();
        cfg.usb_data_role_available = true;
        cfg.usb_data_role = role;
        cfg
    })
    .to_vec()
}

#[test]
fn every_menu_row_fits_its_oled_slot_in_pixels() {
    let mut seen = std::collections::HashSet::new();
    let mut clipped = std::collections::BTreeMap::new();
    for config in representative_help_configs()
        .into_iter()
        .chain(system_configs())
        .chain(behavior_configs())
    {
        let menu = NativeMenuModel::new(config);
        for child in &menu.root.children {
            collect_clipped(child, "Menu", &mut seen, &mut clipped);
        }
    }
    let report: Vec<String> = clipped
        .iter()
        .map(|(row, path)| format!("{row:<36} {path}"))
        .collect();
    assert!(
        clipped.is_empty(),
        "{} clipped rows:\n{}",
        clipped.len(),
        report.join("\n")
    );
}
