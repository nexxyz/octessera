use super::{NativeMenuBarValue, NativeMenuItem, NativeMenuValue};

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
        NativeMenuValue::Group => vec![format_menu_line(&item.label, selected)],
        NativeMenuValue::Action(_) => vec![format_menu_line(&format!("!{}", item.label), selected)],
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
        .map(|line| clip_menu_value(&line, 20))
        .collect()
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

fn format_display_value(key: Option<&str>, value: impl ToString) -> String {
    let raw = value.to_string();
    let Some(key) = key else {
        return raw;
    };
    if key.ends_with("panPos") {
        return format_pan_position(raw.parse::<i32>().unwrap_or(16));
    }
    if key.ends_with("pitch.lowestNote")
        || key.ends_with("pitch.highestNote")
        || key.ends_with("pitch.startingNote")
    {
        return format_note_with_midi(raw.parse::<i32>().unwrap_or(60));
    }
    if key.ends_with("pitch.scale") {
        return format_scale_name(&raw);
    }
    if key.contains(".params.") {
        return format_fx_param_display(key, raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Pct") || key.ends_with("Percent") {
        return format!("{}%", raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Ms") {
        let ms = raw.parse::<i32>().unwrap_or(0);
        if ms.abs() >= 1000 {
            return format!("{:.1}s", ms as f32 / 1000.0);
        }
        return format!("{ms}ms");
    }
    raw
}

fn format_fx_param_display(key: &str, value: i32) -> String {
    if key.ends_with(".decay") {
        return format_reverb_decay_seconds(value as f64 / 1000.0);
    }
    if key.ends_with("Hz") {
        return format_fixed_unit(value as f64 / 100.0, "Hz");
    }
    if key.ends_with("Db") || key.ends_with("GainDb") || key.ends_with("thresholdDb") {
        return format!("{:+.1}dB", value as f64 / 2.0);
    }
    if key.ends_with("feedback")
        || key.ends_with("threshold")
        || key.ends_with("clip")
        || key.ends_with("q")
        || key.ends_with("midQ")
    {
        return format_fixed(value as f64 / 100.0, 2);
    }
    if key.ends_with("drive") || key.ends_with("depthMs") || key.ends_with("baseMs") {
        return format_fixed(value as f64 / 10.0, 1);
    }
    if key.ends_with("ratio") {
        return format_fixed(value as f64 / 2.0, 1);
    }
    if key.ends_with("Pct") {
        return format!("{value}%");
    }
    if key.ends_with("Ms") {
        if value.abs() >= 1000 {
            return format!("{:.1}s", value as f32 / 1000.0);
        }
        return format!("{value}ms");
    }
    value.to_string()
}

fn format_fixed_unit(value: f64, unit: &str) -> String {
    format!("{}{unit}", format_fixed(value, 2))
}

fn format_fixed(value: f64, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_reverb_decay_seconds(value: f64) -> String {
    let feedback = value.clamp(0.0, 0.995);
    if feedback <= 0.0 {
        return "0.0s".into();
    }
    let average_delay_seconds = ((1557.0 + 1617.0 + 1491.0 + 1422.0) / 4.0) / 44_100.0;
    let seconds = (-3.0 * average_delay_seconds) / feedback.log10();
    format!("{seconds:.1}s")
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

fn format_scale_name(value: &str) -> String {
    match value {
        "chromatic" => "Chromatic",
        "major" => "Major",
        "natural_minor" => "Natural Minor",
        "dorian" => "Dorian",
        "mixolydian" => "Mixolydian",
        "major_pentatonic" => "Maj Pentatonic",
        "minor_pentatonic" => "Min Pentatonic",
        "harmonic_minor" => "Harm Minor",
        _ => value,
    }
    .into()
}

fn format_note_with_midi(note: i32) -> String {
    let note = note.clamp(0, 127);
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let name = names[(note % 12) as usize];
    let octave = note / 12 - 1;
    format!("{name}{octave} ({note})")
}

fn format_pan_position(value: i32) -> String {
    let pos = value.clamp(0, 32);
    let distance = pos - 16;
    if distance == 0 {
        "C".into()
    } else if distance < 0 {
        format!("L{}", distance.abs().min(15))
    } else {
        format!("R{}", distance.min(15))
    }
}

fn format_param_lines(
    label: &str,
    value: impl Into<String>,
    selected: bool,
    editing: bool,
) -> Vec<String> {
    let value = value.into();
    if selected {
        if !editing {
            return vec![format_selected_param_line(label, &value)];
        }
        let marker = if editing { "* " } else { "" };
        vec![
            format_menu_line(&format!("{label}:"), true),
            format!("    {marker}{value}"),
        ]
    } else {
        vec![format_menu_line(label, false)]
    }
}

fn format_selected_param_line(label: &str, value: &str) -> String {
    if value.is_empty() {
        return format_menu_line(label, true);
    }
    let width = 18;
    let value_len = value.chars().count();
    if value_len + 1 >= width {
        return format_menu_line(&clip_menu_value(value, width), true);
    }
    let label_width = width - value_len - 1;
    format_menu_line(
        &format!("{} {value}", clip_menu_value(label, label_width)),
        true,
    )
}

fn format_text_lines(
    label: &str,
    value: &str,
    _cursor: usize,
    selected: bool,
    editing: bool,
) -> Vec<String> {
    let display = if value.is_empty() { "(empty)" } else { value };
    if selected {
        if !editing {
            return vec![format_menu_line(
                &format!("{label} {}", clip_menu_value(display, 22)),
                true,
            )];
        }
        let marker = if editing { "* " } else { "" };
        vec![
            format_menu_line(&format!("{label}:"), true),
            format!("    {marker}{}", clip_menu_value(display, 22)),
        ]
    } else {
        vec![format_menu_line(label, false)]
    }
}

fn format_menu_line(text: &str, selected: bool) -> String {
    if selected {
        format!("> {text}")
    } else {
        format!("  {text}")
    }
}

fn clip_menu_value(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    format!("{}...", value.chars().take(width - 3).collect::<String>())
}

pub(super) fn abbreviate_path(path: &str) -> String {
    let mut out = Vec::new();
    for segment in path.split('/') {
        out.push(match segment {
            "Menu" => "MENU".to_string(),
            "System" => "SYS".to_string(),
            other => other.to_string(),
        });
    }
    out.join("/")
}

pub(super) fn section_color_from_path(path: &str) -> u16 {
    let first = path.split('/').next().unwrap_or("MENU");
    section_color_for_label(first)
}

pub(super) fn section_color_for_label(label: &str) -> u16 {
    if label.starts_with("L1:") || label == "L1: Life" {
        return 0x8ED1;
    }
    if label.starts_with("L2:") || label == "L2: Sense" {
        return 0x8D5C;
    }
    if label.starts_with("L3:") || label == "L3: Voice" {
        return 0xC59B;
    }
    if label.starts_with("L4:") || label == "L4: Dance" {
        return 0xFFFF;
    }
    if label == "System" || label == "SYS" || label == "MENU" {
        return 0xB50D;
    }
    0xFFFF
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
