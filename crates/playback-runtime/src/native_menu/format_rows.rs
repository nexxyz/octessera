pub(in crate::native_menu) fn format_param_lines(
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
        vec![format_unselected_param_line(label, &value)]
    }
}

pub(in crate::native_menu) fn format_full_param_line(label: &str, value: &str) -> String {
    if value.is_empty() {
        return format_menu_line(label, true);
    }
    format_menu_line(&format!("{label} {value}"), true)
}

fn format_unselected_param_line(label: &str, value: &str) -> String {
    if value.is_empty() {
        return format_menu_line(label, false);
    }
    let width = 18;
    let value_len = value.chars().count();
    if value_len + 1 >= width {
        return format_menu_line(&clip_menu_value(value, width), false);
    }
    let label_width = width - value_len - 1;
    format_menu_line(
        &format!("{} {value}", clip_menu_value(label, label_width)),
        false,
    )
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

pub(in crate::native_menu) fn format_text_lines(
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
        vec![format_menu_line(
            &format!("{label} {}", clip_menu_value(display, 22)),
            false,
        )]
    }
}

pub(in crate::native_menu) fn format_menu_line(text: &str, selected: bool) -> String {
    if selected {
        format!("> {text}")
    } else {
        format!("  {text}")
    }
}

pub(in crate::native_menu) fn clip_menu_value(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    format!("{}...", value.chars().take(width - 3).collect::<String>())
}
