use crate::native_menu::section_labels::{
    BUILD_LABEL, BUILD_SHORT_LABEL, LINK_LABEL, LINK_SHORT_LABEL, PLAY_LABEL, PLAY_SHORT_LABEL,
    SHAPE_LABEL, SHAPE_SHORT_LABEL,
};

const BREADCRUMB_COLUMNS: usize = 15;

pub(in crate::native_menu) fn abbreviate_path(path: &str) -> String {
    let segments = path.split('/').collect::<Vec<_>>();
    let last_index = segments.len().saturating_sub(1);
    let labels = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| path_segment_label(segment, index < last_index))
        .collect::<Vec<_>>();
    fit_breadcrumb(&labels)
}

fn path_segment_label(segment: &str, short: bool) -> String {
    if short {
        return short_path_segment_label(segment);
    }
    match segment {
        "Menu" => "MENU".to_string(),
        other => other.to_string(),
    }
}

fn short_path_segment_label(segment: &str) -> String {
    match segment {
        "Menu" => "MENU".to_string(),
        BUILD_LABEL => BUILD_SHORT_LABEL.to_string(),
        LINK_LABEL => LINK_SHORT_LABEL.to_string(),
        SHAPE_LABEL => SHAPE_SHORT_LABEL.to_string(),
        PLAY_LABEL => PLAY_SHORT_LABEL.to_string(),
        "System" => "SYS".to_string(),
        other if other.starts_with("Bus ") => other.replacen("Bus ", "B", 1),
        other if other.starts_with("Layer ") => other.replacen("Layer ", "L", 1),
        other => other.to_string(),
    }
}

fn fit_breadcrumb(labels: &[String]) -> String {
    let full = format!("/{}", labels.join("/"));
    if full.chars().count() <= BREADCRUMB_COLUMNS {
        return full;
    }
    for start in 1..labels.len() {
        let tail = format!("../{}", labels[start..].join("/"));
        if tail.chars().count() <= BREADCRUMB_COLUMNS {
            return tail;
        }
    }
    labels.last().cloned().unwrap_or_default()
}

pub(in crate::native_menu) fn section_color_from_path(path: &str) -> u16 {
    let first = path
        .trim_start_matches("...")
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or("MENU");
    section_color_for_label(first)
}

pub(in crate::native_menu) fn section_color_for_label(label: &str) -> u16 {
    if label == BUILD_LABEL || label == BUILD_SHORT_LABEL {
        return platform_core::palette::GREEN_RGB565;
    }
    if label == LINK_LABEL || label == LINK_SHORT_LABEL {
        return platform_core::palette::MAGENTA_RGB565;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breadcrumbs_use_full_current_and_short_ancestors() {
        assert_eq!(abbreviate_path("Shape/FX/Bus 1"), "/S/FX/Bus 1");
        assert_eq!(abbreviate_path("Shape/FX/Bus 1/Slot 1"), "/S/FX/B1/Slot 1");
        assert_eq!(abbreviate_path("Build/Layer 3/Behavior"), "/B/L3/Behavior");
    }

    #[test]
    fn breadcrumbs_drop_parents_from_the_left_and_keep_the_page() {
        assert_eq!(
            abbreviate_path("Shape/Instruments/I1: synth/Synth/Filter"),
            "../Synth/Filter"
        );
        assert_eq!(
            abbreviate_path("Shape/Instruments/I1: synth/Synth/Filter Env"),
            "../Filter Env"
        );
    }

    #[test]
    fn breadcrumbs_show_only_the_page_when_dots_do_not_fit() {
        assert_eq!(
            abbreviate_path("Build/Layer 1/Behavior: life"),
            "Behavior: life"
        );
        assert_eq!(
            abbreviate_path("System/A Page Name Beyond Fifteen"),
            "A Page Name Beyond Fifteen"
        );
    }

    #[test]
    fn shortened_breadcrumb_uses_canonical_section_color() {
        let path = "Shape/FX/Bus 1/Slot 1/Very Long Name";
        assert_eq!(abbreviate_path(path), "Very Long Name");
        assert_eq!(
            section_color_from_path(path),
            platform_core::palette::BLUE_RGB565
        );
    }
}
