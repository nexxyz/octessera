use super::aux_auto_map::{AuxBindingSource, ResolvedAuxSlot};
use super::*;
use crate::native_menu::section_labels::{BUILD_LABEL, PLAY_LABEL};
use crate::oled_frame::{wrap_text, MENU_BODY_RECT};

impl NativeRunner {
    pub(super) fn aux_mapping_popup(&self) -> NativeHelpPopup {
        let normal_slots = self.overlay_aux_slots();
        let shifted_slots = (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| self.resolve_shift_aux_slot(index))
            .collect::<Vec<_>>();
        let mut lines = Vec::new();
        let context = self.aux_overlay_context_label();
        if context.chars().count() <= MENU_BODY_RECT.columns() {
            lines.push(context);
        }
        append_aux_mapping_lines(&mut lines, &normal_slots, false);
        append_aux_mapping_lines(&mut lines, &shifted_slots, true);
        NativeHelpPopup {
            title: overlay_title().into(),
            lines,
            scroll: 0,
        }
    }

    fn aux_overlay_context_label(&self) -> String {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        if let Some(key) = selected_key {
            if let Some(label) = aux_overlay_key_context_label(&key) {
                return label.into();
            }
        }
        if matches!(selected_action, Some(NativeMenuAction::PlatformEffect(action)) if action.starts_with("sample.assign:"))
        {
            return "Sample".into();
        }
        if path.contains(BUILD_LABEL) {
            return BUILD_LABEL.into();
        }
        if path.contains(PLAY_LABEL) {
            return "Play FX".into();
        }
        "Aux Map".into()
    }

    fn overlay_aux_slots(&self) -> Vec<ResolvedAuxSlot> {
        (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| self.effective_aux_slot(index))
            .collect()
    }
}

fn append_aux_mapping_lines(lines: &mut Vec<String>, slots: &[ResolvedAuxSlot], shifted: bool) {
    for (index, slot) in slots.iter().enumerate() {
        append_aux_binding_line(
            lines,
            index,
            shifted,
            'T',
            slot.turn.as_ref().map(|turn| turn.label.as_str()),
            slot.turn_source,
        );
        append_aux_binding_line(
            lines,
            index,
            shifted,
            'C',
            slot.press.as_ref().map(|press| press.label.as_str()),
            slot.press_source,
        );
    }
}

fn append_aux_binding_line(
    lines: &mut Vec<String>,
    index: usize,
    shifted: bool,
    kind: char,
    label: Option<&str>,
    source: AuxBindingSource,
) {
    let prefix = format!("{}{} {kind} ", if shifted { 'S' } else { 'A' }, index + 1);
    let value = match (label, source) {
        (Some(label), AuxBindingSource::Auto) => format!("auto: {label}"),
        (Some(label), AuxBindingSource::Custom) => format!("custom: {label}"),
        _ => "-".into(),
    };
    let content_width = MENU_BODY_RECT.columns() - prefix.chars().count();
    lines.extend(
        wrap_text(&value, content_width)
            .into_iter()
            .map(|row| format!("{prefix}{row}")),
    );
}

fn overlay_title() -> &'static str {
    "AUX MAP"
}

fn aux_overlay_key_context_label(key: &str) -> Option<&'static str> {
    aux_overlay_filter_context(key)
        .or_else(|| aux_overlay_env_context(key))
        .or_else(|| aux_overlay_oscillator_context(key))
        .or_else(|| aux_overlay_fx_context(key))
        .or_else(|| aux_overlay_behavior_context(key))
}

fn aux_overlay_filter_context(key: &str) -> Option<&'static str> {
    if key.contains(".synth.filter.") {
        Some("Synth Filter")
    } else if key.contains(".sample.filter.") {
        Some("Sample Filter")
    } else {
        None
    }
}

fn aux_overlay_env_context(key: &str) -> Option<&'static str> {
    if key.contains(".synth.ampEnv.") || key.contains(".sample.ampEnv.") {
        Some("Amp Env")
    } else if key.contains(".synth.filterEnv.") || key.contains(".sample.filterEnv.") {
        Some("Filter Env")
    } else {
        None
    }
}

fn aux_overlay_oscillator_context(key: &str) -> Option<&'static str> {
    if key.contains(".synth.osc1.") {
        Some("Osc 1")
    } else if key.contains(".synth.osc2.") {
        Some("Osc 2")
    } else {
        None
    }
}

fn aux_overlay_fx_context(key: &str) -> Option<&'static str> {
    if key.contains("mixer.buses.") {
        Some("FX Bus")
    } else if key.contains("mixer.master.slots.") {
        Some("Global FX")
    } else if key.contains("play.fx.params.") {
        Some("Play FX")
    } else {
        None
    }
}

fn aux_overlay_behavior_context(key: &str) -> Option<&'static str> {
    if key.starts_with("layers.") && key.contains(".behaviorConfig.") {
        Some(BUILD_LABEL)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_runner::aux_auto_map::{ResolvedAuxPress, ResolvedAuxTurn};

    fn slot(
        turn_label: Option<&str>,
        turn_source: AuxBindingSource,
        press_label: Option<&str>,
        press_source: AuxBindingSource,
    ) -> ResolvedAuxSlot {
        ResolvedAuxSlot {
            turn: turn_label.map(|label| ResolvedAuxTurn {
                key: String::new(),
                label: label.into(),
            }),
            press: press_label.map(|label| ResolvedAuxPress {
                action: NativeMenuAction::NavigateBack,
                label: label.into(),
            }),
            turn_source,
            press_source,
        }
    }

    #[test]
    fn aux_mapping_rows_keep_all_entries_sources_and_width() {
        let slots = vec![
            slot(
                Some("Cutoff"),
                AuxBindingSource::Auto,
                None,
                AuxBindingSource::None,
            ),
            slot(
                Some("Resonance"),
                AuxBindingSource::Custom,
                Some("Panic"),
                AuxBindingSource::Custom,
            ),
            slot(None, AuxBindingSource::None, None, AuxBindingSource::None),
        ];
        let mut lines = Vec::new();
        append_aux_mapping_lines(&mut lines, &slots, false);
        append_aux_mapping_lines(&mut lines, &slots, true);

        for bank in ['A', 'S'] {
            for index in 1..=3 {
                for kind in ['T', 'C'] {
                    let prefix = format!("{bank}{index} {kind} ");
                    assert!(lines.iter().any(|line| line.starts_with(&prefix)));
                }
            }
        }
        assert!(lines
            .iter()
            .all(|line| line.chars().count() <= MENU_BODY_RECT.columns()));
        assert!(lines.iter().any(|line| line == "A1 T auto: Cutoff"));
        assert!(lines.iter().any(|line| line == "A1 C -"));
        assert!(lines.iter().any(|line| line == "A2 T custom:"));
        assert!(lines.iter().any(|line| line == "A2 T Resonance"));
        assert!(lines.iter().any(|line| line == "A2 C custom: Panic"));
        assert!(lines.iter().any(|line| line == "S2 C custom: Panic"));
        assert!(lines.iter().any(|line| line == "S3 T -"));
    }

    #[test]
    fn long_aux_mapping_label_wraps_with_identity_and_content() {
        let label = "Long MIDI Panic Mapping Label";
        let slots = [slot(
            Some(label),
            AuxBindingSource::Custom,
            None,
            AuxBindingSource::None,
        )];
        let mut lines = Vec::new();
        append_aux_mapping_lines(&mut lines, &slots, false);

        let wrapped = lines
            .iter()
            .filter(|line| line.starts_with("A1 T "))
            .collect::<Vec<_>>();
        assert!(wrapped.len() > 1);
        assert!(wrapped
            .iter()
            .all(|line| line.chars().count() <= MENU_BODY_RECT.columns()));
        assert_eq!(
            wrapped
                .iter()
                .map(|line| line.strip_prefix("A1 T ").unwrap())
                .collect::<Vec<_>>()
                .join(" "),
            format!("custom: {label}")
        );
    }
}
