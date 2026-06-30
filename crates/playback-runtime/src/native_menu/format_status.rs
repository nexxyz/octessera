pub(in crate::native_menu) fn abbreviate_path(path: &str) -> String {
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

pub(in crate::native_menu) fn section_color_from_path(path: &str) -> u16 {
    let first = path.split('/').next().unwrap_or("MENU");
    section_color_for_label(first)
}

pub(in crate::native_menu) fn section_color_for_label(label: &str) -> u16 {
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
