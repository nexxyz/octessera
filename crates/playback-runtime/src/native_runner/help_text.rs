/// Overlay text wrapped to the OLED's plain body lines.
pub(super) fn wrap_help_text(text: &str) -> Vec<String> {
    let lines = crate::oled_frame::wrap_body_text(text);
    if lines.is_empty() {
        return vec!["No help available.".into()];
    }
    lines
}
