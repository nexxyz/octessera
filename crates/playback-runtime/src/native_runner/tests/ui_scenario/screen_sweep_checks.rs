//! What every swept screen must satisfy: fixed text fits, names scroll, help is specific and
//! values show display labels.

use crate::native_help::resolve_native_help_entry;
use crate::native_menu::{NativeMenuAction, NativeMenuHelpTarget, NativeMenuItem, NativeMenuValue};
use crate::oled_frame::{
    menu_row_text_overflows, rows_show_aux_caps, OledDisplayLayout, OledMenuRowInput,
    OledMenuRowKind, OledPresentationInput,
};

/// Fixture names the sweep feeds the runner: preset, sample and MIDI port names may clip.
pub(super) const FIXTURE_NAMES: &[&str] = &[
    "late-night-ambient-session-with-drums",
    "Field recordings from the old train station",
    "165028__rodrigo-the-mad__mini-909ish-open-hat",
    "a-really-long-sample-file-name-take-03",
    "Arturia KeyStep 37 MIDI 1",
    "Elektron Digitakt II USB MIDI Port 1 (very long)",
];

/// The problems of one rendered screen: each clipped text that is no name.
pub(super) fn clip_failures(
    input: &OledPresentationInput,
    items: &[Option<&NativeMenuItem>],
    clipped: &[String],
) -> Vec<String> {
    let display = &input.display;
    clipped
        .iter()
        .filter(|text| **text != display.toast)
        .filter_map(|text| {
            let row = display.rows.iter().enumerate().find_map(|(index, row)| {
                let row = row.as_ref()?;
                row_shows(row, text).then(|| (row, items.get(index).copied().flatten()))
            });
            match row {
                Some((row, item)) if clipped_name(row, item, text) => None,
                Some((row, _)) => Some(format!(
                    "fixed row text clips: `{}` `{}`",
                    row.label, row.value
                )),
                None if display.lines.iter().any(|line| line == text) => {
                    Some(format!("overlay line clips: `{text}`"))
                }
                None => Some(format!("header clips: `{text}`")),
            }
        })
        .collect()
}

fn row_shows(row: &OledMenuRowInput, text: &str) -> bool {
    text == row.label
        || text == row.value
        || text == format!("{}: ", row.label)
        || text == format!("{}: {}", row.label, row.value)
}

/// Whether the clipped `text` of `row` is a name: a file, preset, port, layer, instrument, bus
/// or behavior name, a text value, or a value picked from a list.
fn clipped_name(row: &OledMenuRowInput, item: Option<&NativeMenuItem>, text: &str) -> bool {
    let value_part = !row.value.is_empty() && text.ends_with(row.value.as_str());
    if value_part
        && (row.kind == OledMenuRowKind::Picker
            || row.kind == OledMenuRowKind::Group
            || matches!(
                item.map(|item| &item.value),
                Some(NativeMenuValue::Text { .. })
            )
            || is_slot_name(&row.value))
    {
        return true;
    }
    row.kind == OledMenuRowKind::Entry || is_slot_name(&row.label) || is_named(&row.label)
}

/// `L1: life`, `I3: Plucked fxb1`, `B2: Delay+Reverb`, `S1: kick.wav`: a slot and its name.
fn is_slot_name(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('L' | 'I' | 'B' | 'S'))
        && chars.next().is_some_and(|ch| ch.is_ascii_digit())
        && text.contains(": ")
}

fn is_named(text: &str) -> bool {
    FIXTURE_NAMES.iter().any(|name| text.contains(name))
        || platform_core::behavior_catalog()
            .iter()
            .any(|entry| entry.label == text)
}

/// Whether the selected row is wider than its slot, so it must scroll.
pub(super) fn selected_row_overflows(input: &OledPresentationInput) -> bool {
    let display = &input.display;
    let Some(index) = input.selected_row else {
        return false;
    };
    let Some(Some(row)) = display.rows.get(index) else {
        return false;
    };
    let has_bar = display.bars.get(index).is_some_and(Option::is_some);
    let gutter =
        display.body_layout == OledDisplayLayout::Rows && rows_show_aux_caps(&display.rows);
    menu_row_text_overflows(row, has_bar, gutter, display.main_font)
}

/// The help problems of the selected row: no target, no entry, or a clear-binding entry on a
/// row that does not clear.
pub(super) fn help_failure(target: Option<&NativeMenuHelpTarget>) -> Option<String> {
    let Some(target) = target else {
        return Some("no help target".into());
    };
    let Some(entry) = resolve_native_help_entry(target) else {
        return Some(format!("Missing help: `{}` ({})", target.label, target.key));
    };
    let clears = matches!(
        entry.key.as_str(),
        "action:param_clear" | "action:aux_click_set_target" | "action:fader_tile_set_action"
    );
    (clears && target.label != "(none)").then(|| {
        format!(
            "`{}` shows the clear-binding help `{}`",
            target.label, entry.title
        )
    })
}

/// The value problems of the selected row: a raw `snake_case` id, or a bare number standing in
/// for a text option.
pub(super) fn value_failure(item: &NativeMenuItem, row: &OledMenuRowInput) -> Option<String> {
    let shown = row.value.as_str();
    match &item.value {
        NativeMenuValue::Enum { options, selected } => {
            let raw = options
                .get(*selected)
                .map(String::as_str)
                .unwrap_or_default();
            if !raw.is_empty()
                && raw.parse::<f64>().is_err()
                && shown.chars().all(|ch| ch.is_ascii_digit())
                && !shown.is_empty()
            {
                return Some(format!("`{}` shows `{shown}` for `{raw}`", item.label));
            }
        }
        NativeMenuValue::Number { .. } | NativeMenuValue::Bool { .. } => {}
        _ => return None,
    }
    let snake = shown
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| {
            word.contains('_') && word.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')
        });
    snake.then(|| format!("`{}` shows the raw id `{shown}`", item.label))
}

/// Whether `action` opens a confirm dialog or a page the sweep walks into.
pub(super) fn walks_into(action: &NativeMenuAction, label: &str) -> bool {
    let NativeMenuAction::PlatformEffect(effect) = action else {
        return false;
    };
    effect.starts_with("sample.choose:")
        || effect.starts_with("sample.open")
        || effect.starts_with("sample.loaded:")
        || effect.starts_with("sample.favorite.open")
        || effect == "system.controlsHelp"
        || effect == "system.info"
        || effect.starts_with("sample.pick") && label.contains("rodrigo")
}
