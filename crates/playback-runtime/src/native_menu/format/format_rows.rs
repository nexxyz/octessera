pub(in crate::native_menu) fn format_full_param_line(label: &str, value: &str) -> String {
    if value.is_empty() {
        return format_menu_line(label);
    }
    format_menu_line(&format!("{label} {value}"))
}

pub(in crate::native_menu) fn format_param_line(label: &str, value: &str) -> String {
    if value.is_empty() {
        return format_menu_line(label);
    }
    let width = 18;
    let value_len = value.chars().count();
    if value_len + 1 >= width {
        return format_menu_line(&clip_menu_value(value, width));
    }
    let label_width = width - value_len - 1;
    format_menu_line(&format!("{} {value}", clip_menu_value(label, label_width)))
}

pub(in crate::native_menu) fn format_menu_line(text: &str) -> String {
    format!("  {text}")
}

pub(in crate::native_menu) fn format_action_menu_line(text: &str) -> String {
    format!(" !{text}")
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
