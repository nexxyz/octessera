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
        return 0x8ED1;
    }
    if label.starts_with("2:") || label == "P" {
        return 0x8D5C;
    }
    if label.starts_with("3:") || label == "T" {
        return 0xC59B;
    }
    if label.starts_with("4:") || label == "S" {
        return 0xFFFF;
    }
    if label == "System" || label == "SYS" || label == "MENU" {
        return 0xB50D;
    }
    0xFFFF
}
