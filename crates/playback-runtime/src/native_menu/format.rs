use super::{
    NativeMenuBarValue, NativeMenuItem, NativeMenuRow, NativeMenuRowKind, NativeMenuValue,
};
use crate::native_menu::format_values::format_display_value;

mod format_rows;
mod format_status;

use format_rows::{
    clip_menu_value, format_action_menu_line, format_full_param_line, format_menu_line,
    format_param_line,
};
pub(in crate::native_menu) use format_status::{
    abbreviate_path, section_color_for_label, section_color_from_path,
};

/// The single body row of an item: its text line and, for labelled items, its parts.
pub(super) fn format_item_row(
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
) -> (String, Option<NativeMenuRow>) {
    if item.label.is_empty() {
        return (String::new(), None);
    }
    let row = item_row_parts(item, selected, editing);
    let line = match (&row.kind, &item.value) {
        (NativeMenuRowKind::Action, _) => format_action_menu_line(&row.label),
        (NativeMenuRowKind::Group, _) if row.value.is_empty() => {
            format_menu_line(&format!("{} >", row.label))
        }
        (NativeMenuRowKind::Group, _) => {
            format_menu_line(&format!("{}: {} >", row.label, row.value))
        }
        (_, NativeMenuValue::Group) => format_menu_line(&row.label),
        (_, NativeMenuValue::Text { .. }) => format_menu_line(&format!(
            "{} {}",
            row.label,
            clip_menu_value(&row.value, 22)
        )),
        _ => format_param_line(&row.label, &row.value),
    };
    let line = if selected {
        line
    } else {
        clip_menu_value(&line, 20)
    };
    (line, Some(row))
}

fn item_row_parts(item: &NativeMenuItem, selected: bool, editing: bool) -> NativeMenuRow {
    let row = |kind, value: String| NativeMenuRow {
        kind,
        label: item.label.clone(),
        value,
        text_cursor: None,
    };
    let option = |options: &[String], current: usize| {
        format_display_value(
            item.key.as_deref(),
            options.get(current).cloned().unwrap_or_default(),
        )
    };
    match &item.value {
        NativeMenuValue::Group if item.children.is_empty() => {
            row(NativeMenuRowKind::Value, String::new())
        }
        NativeMenuValue::Group => row(NativeMenuRowKind::Group, String::new()),
        NativeMenuValue::Action(_) => row(NativeMenuRowKind::Action, String::new()),
        NativeMenuValue::Enum {
            options,
            selected: current,
        } if !item.children.is_empty() => row(NativeMenuRowKind::Group, option(options, *current)),
        NativeMenuValue::Enum {
            options,
            selected: current,
        } if is_link_instrument_target(item) => {
            let value = options.get(*current).cloned().unwrap_or_default();
            let shown = if selected {
                value.as_str()
            } else {
                value
                    .split_once(':')
                    .map_or(value.as_str(), |(slot, _)| slot)
            };
            row(NativeMenuRowKind::Value, shown.into())
        }
        NativeMenuValue::Enum {
            options,
            selected: current,
        } => row(NativeMenuRowKind::Value, option(options, *current)),
        NativeMenuValue::Number { value, .. } => row(
            NativeMenuRowKind::Value,
            format_display_value(item.key.as_deref(), *value),
        ),
        NativeMenuValue::Bool { value } => row(
            NativeMenuRowKind::Value,
            if *value { "On" } else { "Off" }.into(),
        ),
        NativeMenuValue::Text { value, cursor, .. } if selected && editing => NativeMenuRow {
            text_cursor: Some(*cursor),
            ..row(NativeMenuRowKind::Value, value.clone())
        },
        NativeMenuValue::Text { value, .. } => row(
            NativeMenuRowKind::Value,
            if value.is_empty() { "(empty)" } else { value }.into(),
        ),
    }
}

pub(super) fn is_link_instrument_target(item: &NativeMenuItem) -> bool {
    item.key.as_deref().is_some_and(|key| {
        key.starts_with("layers.") && key.contains(".link.mapping.") && key.ends_with(".slot")
    })
}

pub(super) fn format_item_full_selected_line(item: &NativeMenuItem) -> Option<String> {
    match &item.value {
        NativeMenuValue::Group => {
            let label = if item.children.is_empty() {
                item.label.clone()
            } else {
                format!("{} >", item.label)
            };
            Some(format_menu_line(&label))
        }
        NativeMenuValue::Enum { options, selected } if !item.children.is_empty() => {
            Some(format_menu_line(&format!(
                "{}: {} >",
                item.label,
                format_display_value(
                    item.key.as_deref(),
                    options.get(*selected).cloned().unwrap_or_default(),
                )
            )))
        }
        NativeMenuValue::Enum { options, selected } => Some(format_full_param_line(
            &item.label,
            &format_display_value(
                item.key.as_deref(),
                options.get(*selected).cloned().unwrap_or_default(),
            ),
        )),
        NativeMenuValue::Number { value, .. } => Some(format_full_param_line(
            &item.label,
            &format_display_value(item.key.as_deref(), *value),
        )),
        NativeMenuValue::Bool { value } => Some(format_full_param_line(
            &item.label,
            if *value { "On" } else { "Off" },
        )),
        NativeMenuValue::Text { value, .. } => Some(format_menu_line(&format!(
            "{} {}",
            item.label,
            if value.is_empty() { "(empty)" } else { value }
        ))),
        NativeMenuValue::Action(_) => item
            .key
            .as_deref()
            .and_then(|key| key.strip_prefix("sample.loaded:"))
            .and_then(|rest| rest.splitn(3, ':').nth(2))
            .map(|path| {
                let display = if item.label.starts_with("N/A-") {
                    item.label.as_str()
                } else {
                    path
                };
                format_action_menu_line(display)
            })
            .or_else(|| Some(format_action_menu_line(&item.label))),
    }
}

pub(super) fn format_item_bar_value(
    item: &NativeMenuItem,
    selected: bool,
    numeric_display_mode: &str,
) -> Option<NativeMenuBarValue> {
    if numeric_display_mode == "numbers" || !selected {
        return None;
    }
    let NativeMenuValue::Number {
        value, min, max, ..
    } = item.value
    else {
        return None;
    };
    let key = item.key.as_deref()?;
    if !should_use_number_bar(key) {
        return None;
    }
    let range = (max - min).max(1);
    let frac_pct = ((((value - min).clamp(0, range) as f32 / range as f32) * 100.0).round()) as u8;
    Some(NativeMenuBarValue {
        frac_pct,
        num_chars: if numeric_display_mode == "bar" {
            0
        } else {
            bar_number_chars(key, min, max)
        },
        style: if is_marker_bar_key(key) {
            Some("marker".into())
        } else {
            None
        },
    })
}

fn should_use_number_bar(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    if key_lower.ends_with("channel")
        || key_lower.ends_with("selectedslot")
        || key_lower.ends_with("startingnote")
        || key_lower.ends_with("lowestnote")
        || key_lower.ends_with("highestnote")
    {
        return false;
    }
    key == "masterVolume"
        || key == "transport.bpm"
        || key == "dimTimerSeconds"
        || key == "screenSleepSeconds"
        || key_lower.ends_with(".fm.index")
        || key.contains(".params.")
        || key_lower.ends_with("pct")
        || key_lower.ends_with("percent")
        || key_lower.ends_with("ms")
        || key_lower.ends_with("hz")
        || key_lower.ends_with("db")
        || key_lower.ends_with("semis")
        || key_lower.ends_with("semitones")
        || key_lower.ends_with("cents")
        || key_lower.ends_with("panpos")
        || key_lower.ends_with("volume")
        || key_lower.ends_with("basevelocity")
        || key_lower.ends_with("velocity")
        || key_lower.ends_with("high")
        || key_lower.ends_with("medium")
        || key_lower.ends_with("low")
        || key_lower.ends_with("durationms")
        || key_lower.ends_with("gainpct")
        || key_lower.ends_with("velocitysensitivitypct")
        || key_lower.ends_with("levelpct")
        || key_lower.ends_with("pulsewidthpct")
        || key_lower.ends_with("detunecents")
        || key_lower.ends_with("cutoffhz")
        || key_lower.ends_with("resonance")
        || key_lower.ends_with("envamountpct")
        || key_lower.ends_with("keytrackingpct")
        || key_lower.ends_with("attackms")
        || key_lower.ends_with("decayms")
        || key_lower.ends_with("sustainpct")
        || key_lower.ends_with("releasems")
        || key_lower.ends_with("notelengthms")
        || key_lower.ends_with("velocityscalepct")
        || key_lower.ends_with("steps")
        || key_lower.ends_with("from")
        || key_lower.ends_with("to")
        || key_lower.ends_with("gridoffset")
        || key_lower.ends_with("randomcellspertick")
        || key_lower.ends_with("randomtickinterval")
        || key_lower.ends_with("spawnstep")
        || key_lower.ends_with("seedinterval")
        || key_lower.ends_with("randomseedcells")
        || key_lower.ends_with("firethreshold")
        || key_lower.ends_with("maxants")
        || key_lower.ends_with("autospawninterval")
        || key_lower.ends_with("spawninterval")
        || key_lower.ends_with("maxballs")
        || key_lower.ends_with("lifespan")
        || key_lower.ends_with("maxradius")
        || key_lower.ends_with("autopulseinterval")
        || key_lower.ends_with("autodropinterval")
        || key_lower.ends_with("splashradius")
}

fn bar_number_chars(key: &str, min: i32, max: i32) -> usize {
    [min, (min + max) / 2, max]
        .into_iter()
        .map(|value| format_display_value(Some(key), value).len())
        .max()
        .unwrap_or(0)
}

fn is_marker_bar_key(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    key_lower.ends_with("panpos")
        || key_lower.ends_with("semis")
        || key_lower.ends_with("semitones")
        || key_lower.ends_with("cents")
        || key_lower.ends_with("detunecents")
        || key_lower.ends_with("envamountpct")
}

pub(super) fn note_unit_to_pulses(value: &str) -> Option<u32> {
    crate::timing_units::note_unit_to_pulses_option(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_menu::{NativeMenuAction, NativeMenuItem, NativeMenuValue};

    fn action_item(label: &str) -> NativeMenuItem {
        NativeMenuItem {
            label: label.into(),
            breadcrumb: None,
            key: None,
            value: NativeMenuValue::Action(NativeMenuAction::PlatformEffect("test".into())),
            children: Vec::new(),
        }
    }

    #[test]
    fn plain_action_rows_reduce_bang_indent() {
        let item = action_item("Do It");
        assert_eq!(format_item_row(&item, true, false).0, " !Do It");
        assert_eq!(format_item_row(&item, false, false).0, " !Do It");
    }
}
