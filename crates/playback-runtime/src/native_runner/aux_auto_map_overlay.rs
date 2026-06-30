use std::time::Duration;

use super::aux_auto_map::{AuxBindingSource, ResolvedAuxSlot};
use super::*;

const AUX_OVERLAY_DELAY_MS: u64 = 1_500;

impl NativeRunner {
    pub(super) fn aux_mapping_overlay(&self) -> Option<(String, Vec<String>)> {
        if !self.aux_overlay_ready() {
            return None;
        }
        let slots = self.overlay_aux_slots();
        if slots.iter().all(aux_slot_is_empty) {
            return None;
        }
        let title = overlay_title(&slots);
        let mut lines = vec![self.aux_overlay_context_label()];
        lines.extend(
            slots
                .iter()
                .enumerate()
                .map(|(index, slot)| format!("A{} {}", index + 1, aux_slot_body(slot))),
        );
        Some((title.into(), lines))
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
        if path.contains("L1: Life") {
            return "Life".into();
        }
        if path.contains("L4: Dance") {
            return "Dance FX".into();
        }
        "Aux Map".into()
    }

    fn aux_overlay_ready(&self) -> bool {
        self.ui.fn_held
            && !self.ui.shift_held
            && !self.fn_hold_started_at.is_none_or(|started| {
                Instant::now().duration_since(started) < Duration::from_millis(AUX_OVERLAY_DELAY_MS)
            })
    }

    fn overlay_aux_slots(&self) -> Vec<ResolvedAuxSlot> {
        (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| self.effective_aux_slot(index))
            .collect()
    }
}

fn aux_slot_is_empty(slot: &ResolvedAuxSlot) -> bool {
    slot.turn.is_none() && slot.press.is_none()
}

fn overlay_title(slots: &[ResolvedAuxSlot]) -> &'static str {
    let has_auto = slots.iter().any(|slot| {
        slot.turn_source == AuxBindingSource::Auto || slot.press_source == AuxBindingSource::Auto
    });
    let has_custom = slots.iter().any(|slot| {
        slot.turn_source == AuxBindingSource::Custom
            || slot.press_source == AuxBindingSource::Custom
    });
    if has_auto {
        "AUTO MAP"
    } else if has_custom {
        "CUSTOM MAP"
    } else {
        "AUX MAP"
    }
}

fn aux_slot_body(slot: &ResolvedAuxSlot) -> String {
    let mut parts = Vec::new();
    if let Some(turn) = &slot.turn {
        parts.push(turn.label.clone());
    }
    if let Some(press) = &slot.press {
        parts.push(format!("!{}", press.label));
    }
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join("/")
    }
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
    } else if key.contains("dance.fx.params.") {
        Some("Dance FX")
    } else {
        None
    }
}

fn aux_overlay_behavior_context(key: &str) -> Option<&'static str> {
    if key.starts_with("parts.") && key.contains(".behaviorConfig.") {
        Some("Life")
    } else {
        None
    }
}
