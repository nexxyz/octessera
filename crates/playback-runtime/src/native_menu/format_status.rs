pub(in crate::native_menu) fn abbreviate_path(path: &str) -> String {
    let mut out = Vec::new();
    for segment in path.split('/') {
        out.push(match segment {
            "Menu" => "MENU".to_string(),
            "1: Worlds" => "W".to_string(),
            "2: Pulses" => "P".to_string(),
            "3: Tones" => "T".to_string(),
            "4: Sparks" => "S".to_string(),
            "System" => "SYS".to_string(),
            other => other.to_string(),
        });
    }
    out.join("/")
}

pub(in crate::native_menu) fn section_color_from_path(path: &str) -> u16 {
    let first = path.split('/').next().unwrap_or("MENU");
    section_color_for_label(first)
}

pub(in crate::native_menu) fn section_color_for_label(label: &str) -> u16 {
    if label.starts_with("1:") || label == "W" {
        return platform_core::palette::WORLDS_RGB565;
    }
    if label.starts_with("2:") || label == "P" {
        return platform_core::palette::PULSES_RGB565;
    }
    if label.starts_with("3:") || label == "T" {
        return platform_core::palette::TONES_RGB565;
    }
    if label.starts_with("4:") || label == "S" {
        return platform_core::palette::SPARKS_RGB565;
    }
    if label == "System" || label == "SYS" || label == "MENU" {
        return platform_core::palette::SYSTEM_RGB565;
    }
    platform_core::palette::WHITE_RGB565
}
