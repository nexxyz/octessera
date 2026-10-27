use super::*;
use crate::native_menu::format::{format_item_lines, is_link_instrument_target};

fn rendered_values(item: &NativeMenuItem) -> Vec<NativeMenuValue> {
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
        } => [*min, *max]
            .into_iter()
            .map(|value| NativeMenuValue::Number {
                value,
                min: *min,
                max: *max,
                step: *step,
                curve: *curve,
            })
            .collect(),
        NativeMenuValue::Text { .. } => Vec::new(),
        other => vec![other.clone()],
    }
}

fn collect_clipped(
    item: &NativeMenuItem,
    path: &str,
    clipped: &mut std::collections::BTreeMap<String, String>,
) {
    let path = format!("{path} > {}", item.label);
    if !is_link_instrument_target(item) {
        for value in rendered_values(item) {
            let shown = NativeMenuItem {
                value,
                ..item.clone()
            };
            let full = format_item_lines(&shown, true, false, "bar+numbers");
            let row = format_item_lines(&shown, false, false, "bar+numbers");
            if (row[0].contains("...") && !item.label.contains("..."))
                || full[0].trim_start_matches("> ").chars().count() > 18
            {
                clipped
                    .entry(full[0].trim_start_matches("> ").to_string())
                    .or_insert_with(|| {
                        format!("{:?} {}", item.label, item.key.as_deref().unwrap_or("-"))
                    });
            }
        }
    }
    for child in &item.children {
        collect_clipped(child, &path, clipped);
    }
}

#[test]
fn every_menu_row_fits_the_oled_without_clipping() {
    let mut clipped = std::collections::BTreeMap::new();
    for config in representative_help_configs() {
        let menu = NativeMenuModel::new(config);
        for child in &menu.root.children {
            collect_clipped(child, "Menu", &mut clipped);
        }
    }
    let report: Vec<String> = clipped
        .iter()
        .map(|(row, path)| format!("{row:<32} | {path}"))
        .collect();
    assert!(
        clipped.is_empty(),
        "{} clipped rows:
{}",
        clipped.len(),
        report.join(
            "
"
        )
    );
}
