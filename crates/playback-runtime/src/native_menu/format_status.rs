use crate::native_menu::section_labels::{
    BUILD_LABEL, BUILD_SHORT_LABEL, LINK_LABEL, LINK_SHORT_LABEL, PLAY_LABEL, PLAY_SHORT_LABEL,
    SHAPE_LABEL, SHAPE_SHORT_LABEL,
};

pub(in crate::native_menu) fn abbreviate_path(path: &str) -> String {
    let mut out = Vec::new();
    for segment in path.split('/') {
        out.push(match segment {
            "Menu" => "MENU".to_string(),
            BUILD_LABEL => BUILD_SHORT_LABEL.to_string(),
            LINK_LABEL => LINK_SHORT_LABEL.to_string(),
            SHAPE_LABEL => SHAPE_SHORT_LABEL.to_string(),
            PLAY_LABEL => PLAY_SHORT_LABEL.to_string(),
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
    if label == BUILD_LABEL || label == BUILD_SHORT_LABEL {
        return platform_core::palette::GREEN_RGB565;
    }
    if label == LINK_LABEL || label == LINK_SHORT_LABEL {
        return platform_core::palette::RED_RGB565;
    }
    if label == SHAPE_LABEL || label == SHAPE_SHORT_LABEL {
        return platform_core::palette::BLUE_RGB565;
    }
    if label == PLAY_LABEL || label == PLAY_SHORT_LABEL {
        return platform_core::palette::YELLOW_RGB565;
    }
    if label == "System" || label == "SYS" || label == "MENU" {
        return platform_core::palette::GRAY_RGB565;
    }
    platform_core::palette::WHITE_RGB565
}
