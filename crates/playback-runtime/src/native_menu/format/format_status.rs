use crate::native_menu::section_labels::{
    BUILD_LABEL, BUILD_SHORT_LABEL, LINK_LABEL, LINK_SHORT_LABEL, PLAY_LABEL, PLAY_SHORT_LABEL,
    SHAPE_LABEL, SHAPE_SHORT_LABEL,
};

pub(in crate::native_menu) fn abbreviate_path(path: &str) -> String {
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.is_empty() {
        return "MENU".into();
    }
    let last_index = segments.len().saturating_sub(1);
    let out = segments
        .iter()
        .enumerate()
        .map(|(index, segment)| path_segment_label(segment, index < last_index))
        .collect::<Vec<_>>()
        .join("/");
    front_ellipsize(&format!("/{out}"), 28)
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

fn front_ellipsize(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    if width <= 3 {
        return value
            .chars()
            .rev()
            .take(width)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
    if let Some(tail) = slash_boundary_tail(value, width) {
        return tail;
    }
    let tail = value
        .chars()
        .rev()
        .take(width - 3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

fn slash_boundary_tail(value: &str, width: usize) -> Option<String> {
    let segments = value
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    for segment in segments.into_iter().rev() {
        let candidate_len = 3
            + kept
                .iter()
                .map(|part: &&str| part.chars().count() + 1)
                .sum::<usize>()
            + segment.chars().count()
            + 1;
        if candidate_len > width && !kept.is_empty() {
            break;
        }
        kept.push(segment);
    }
    kept.reverse();
    if kept.is_empty() {
        None
    } else {
        Some(format!(".../{}", kept.join("/")))
    }
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
    fn breadcrumbs_front_ellipsize() {
        assert_eq!(
            abbreviate_path("Shape/FX/Bus 1/Slot 1/Very Long Name"),
            ".../B1/Slot 1/Very Long Name"
        );
    }

    #[test]
    fn ellipsized_breadcrumb_uses_canonical_section_color() {
        let path = "Shape/FX/Bus 1/Slot 1/Very Long Name";
        assert!(abbreviate_path(path).starts_with(".../"));
        assert_eq!(
            section_color_from_path(path),
            platform_core::palette::BLUE_RGB565
        );
    }
}
