pub(crate) const BUILD_LABEL: &str = "Build";
pub(crate) const LINK_LABEL: &str = "Link";
pub(crate) const SHAPE_LABEL: &str = "Shape";
pub(crate) const PLAY_LABEL: &str = "Play";
pub(crate) const SYSTEM_LABEL: &str = "System";

pub(crate) const BUILD_SHORT_LABEL: &str = "B";
pub(crate) const LINK_SHORT_LABEL: &str = "L";
pub(crate) const SHAPE_SHORT_LABEL: &str = "S";
pub(crate) const PLAY_SHORT_LABEL: &str = "P";

pub(crate) const PLAY_PATH_PREFIX: &str = "Menu > Play";

/// The menu section that owns a control key or platform action, so a hint for it takes that
/// section's colour wherever it is shown. Play remote pages own no targets of their own.
pub(crate) fn key_home_section(key: &str) -> &'static str {
    let layer_field = key
        .strip_prefix("layers.")
        .and_then(|rest| rest.split_once('.'))
        .map(|(_, field)| field);
    if key.starts_with("play.") {
        PLAY_LABEL
    } else if layer_field
        .is_some_and(|field| field.starts_with("link.") || field.starts_with("paramMods."))
        || key.starts_with("linkLfos.")
        || key.starts_with("transport.")
        || key.starts_with("note_set.")
        || key == "inputEventsWhilePaused"
    {
        LINK_LABEL
    } else if layer_field.is_some()
        || matches!(key, "algorithmStep" | "behaviorId" | "randomSeed")
        || key.starts_with("behavior.")
    {
        BUILD_LABEL
    } else if key.starts_with("instruments.")
        || key.starts_with("mixer.")
        || key.starts_with("sample.")
        || key.starts_with("drum.")
        || ["synth.preset", "fm.preset", "pluck.preset"]
            .iter()
            .any(|prefix| key.starts_with(prefix))
    {
        SHAPE_LABEL
    } else {
        SYSTEM_LABEL
    }
}

pub(crate) fn key_home_section_color(key: &str) -> u16 {
    section_color(key_home_section(key))
}

pub(crate) fn section_color(label: &str) -> u16 {
    super::format::section_color_for_label(label)
}
