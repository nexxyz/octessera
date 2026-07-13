use super::{NativeMenuBarValue, NativeMenuItem, NativeMenuValue};
use crate::native_menu::format_values::format_display_value;

#[path = "format_rows.rs"]
mod format_rows;
#[path = "format_status.rs"]
mod format_status;

use format_rows::{
    clip_menu_value, format_action_menu_line, format_full_param_line, format_menu_line,
    format_param_lines, format_text_lines,
};
pub(in crate::native_menu) use format_status::{
    abbreviate_path, section_color_for_label, section_color_from_path,
};

pub(super) fn format_item_lines(
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
    numeric_display_mode: &str,
) -> Vec<String> {
    if item.label.is_empty() {
        return vec![String::new()];
    }
    let lines = match &item.value {
        NativeMenuValue::Group => {
            let label = if item.children.is_empty() {
                item.label.clone()
            } else {
                format!("{} >", item.label)
            };
            vec![format_menu_line(&label, selected)]
        }
        NativeMenuValue::Action(_) => vec![format_action_menu_line(&item.label, selected)],
        NativeMenuValue::Enum {
            options,
            selected: current,
        } => format_param_lines(
            &item.label,
            format_display_value(
                item.key.as_deref(),
                options.get(*current).cloned().unwrap_or_default(),
            ),
            selected,
            editing,
        ),
        NativeMenuValue::Number { value, .. } => format_param_lines(
            &item.label,
            if should_use_number_bar(item.key.as_deref().unwrap_or_default())
                && numeric_display_mode == "bar"
            {
                String::new()
            } else {
                format_display_value(item.key.as_deref(), *value)
            },
            selected,
            editing,
        ),
        NativeMenuValue::Bool { value } => format_param_lines(
            &item.label,
            if *value { "On" } else { "Off" },
            selected,
            editing,
        ),
        NativeMenuValue::Text { value, cursor, .. } => {
            format_text_lines(&item.label, value, *cursor, selected, editing)
        }
    };
    lines
        .into_iter()
        .map(|line| {
            if selected {
                line
            } else {
                clip_menu_value(&line, 20)
            }
        })
        .collect()
}

pub(super) fn format_item_full_selected_line(
    item: &NativeMenuItem,
    numeric_display_mode: &str,
) -> Option<String> {
    match &item.value {
        NativeMenuValue::Enum { options, selected } => Some(format_full_param_line(
            &item.label,
            &format_display_value(
                item.key.as_deref(),
                options.get(*selected).cloned().unwrap_or_default(),
            ),
        )),
        NativeMenuValue::Number { value, .. } => Some(format_full_param_line(
            &item.label,
            &if should_use_number_bar(item.key.as_deref().unwrap_or_default())
                && numeric_display_mode == "bar"
            {
                String::new()
            } else {
                format_display_value(item.key.as_deref(), *value)
            },
        )),
        NativeMenuValue::Bool { value } => Some(format_full_param_line(
            &item.label,
            if *value { "On" } else { "Off" },
        )),
        NativeMenuValue::Text { value, .. } => Some(format_menu_line(
            &format!(
                "{} {}",
                item.label,
                if value.is_empty() { "(empty)" } else { value }
            ),
            true,
        )),
        NativeMenuValue::Action(_) => item
            .key
            .as_deref()
            .and_then(|key| key.strip_prefix("sample.loaded:"))
            .and_then(|rest| rest.splitn(3, ':').nth(2))
            .map(|path| format_action_menu_line(path, true)),
        _ => None,
    }
}

pub(super) fn formatted_item_row_count(
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
) -> usize {
    if item.label.is_empty() {
        return 1;
    }
    match &item.value {
        NativeMenuValue::Enum { .. }
        | NativeMenuValue::Number { .. }
        | NativeMenuValue::Bool { .. }
        | NativeMenuValue::Text { .. }
            if selected && editing =>
        {
            2
        }
        _ => 1,
    }
}

pub(super) fn format_item_bar_values(
    item: &NativeMenuItem,
    item_line_count: usize,
    selected: bool,
    editing: bool,
    numeric_display_mode: &str,
) -> Vec<Option<NativeMenuBarValue>> {
    if numeric_display_mode == "numbers" {
        return vec![None; item_line_count];
    }
    let NativeMenuValue::Number {
        value, min, max, ..
    } = item.value
    else {
        return vec![None; item_line_count];
    };
    let Some(key) = item.key.as_deref() else {
        return vec![None; item_line_count];
    };
    if !should_use_number_bar(key) {
        return vec![None; item_line_count];
    }
    if !selected {
        return vec![None; item_line_count];
    }
    let range = (max - min).max(1);
    let frac_pct = ((((value - min).clamp(0, range) as f32 / range as f32) * 100.0).round()) as u8;
    let bar = Some(NativeMenuBarValue {
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
    });
    if editing && item_line_count > 1 {
        vec![None, bar]
    } else {
        vec![bar]
    }
}

fn should_use_number_bar(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    if key_lower.ends_with("channel")
        || key_lower.ends_with("selectedslot")
        || key_lower.ends_with("activepartindex")
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
    match value {
        "1/16" => Some(6),
        "1/8" => Some(12),
        "1/4" => Some(24),
        "1/2" => Some(48),
        "1/1" => Some(96),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_menu::{NativeMenuAction, NativeMenuItem, NativeMenuValue};

    fn action_item(label: &str) -> NativeMenuItem {
        NativeMenuItem {
            label: label.into(),
            key: None,
            value: NativeMenuValue::Action(NativeMenuAction::PlatformEffect("test".into())),
            children: Vec::new(),
        }
    }

    #[test]
    fn plain_action_rows_reduce_bang_indent() {
        let item = action_item("Do It");
        assert_eq!(
            format_item_lines(&item, true, false, "bar"),
            vec![">!Do It"]
        );
        assert_eq!(
            format_item_lines(&item, false, false, "bar"),
            vec![" !Do It"]
        );
    }
}
