use crate::native_menu::NativeMenuValue;
use std::time::Duration;

use super::*;

const AUX_OVERLAY_DELAY_MS: u64 = 1_500;

#[derive(Clone)]
pub(super) struct ResolvedAuxTurn {
    pub(super) key: String,
    pub(super) label: String,
}

#[derive(Clone)]
pub(super) struct ResolvedAuxPress {
    pub(super) action: NativeMenuAction,
    pub(super) label: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AuxBindingSource {
    Auto,
    Custom,
    None,
}

#[derive(Clone)]
pub(super) struct ResolvedAuxSlot {
    pub(super) turn: Option<ResolvedAuxTurn>,
    pub(super) press: Option<ResolvedAuxPress>,
    pub(super) turn_source: AuxBindingSource,
    pub(super) press_source: AuxBindingSource,
}

impl NativeRunner {
    pub(super) fn aux_mapping_overlay(&self) -> Option<(String, Vec<String>)> {
        if !self.ui.fn_held || self.ui.shift_held {
            return None;
        }
        if self.fn_hold_started_at.is_none_or(|started| {
            Instant::now().duration_since(started) < Duration::from_millis(AUX_OVERLAY_DELAY_MS)
        }) {
            return None;
        }
        let slots = (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| self.effective_aux_slot(index))
            .collect::<Vec<_>>();
        if slots
            .iter()
            .all(|slot| slot.turn.is_none() && slot.press.is_none())
        {
            return None;
        }
        let has_auto = slots.iter().any(|slot| {
            slot.turn_source == AuxBindingSource::Auto
                || slot.press_source == AuxBindingSource::Auto
        });
        let has_custom = slots.iter().any(|slot| {
            slot.turn_source == AuxBindingSource::Custom
                || slot.press_source == AuxBindingSource::Custom
        });
        let title = if has_auto {
            "AUTO MAP"
        } else if has_custom {
            "CUSTOM MAP"
        } else {
            "AUX MAP"
        };
        let mut lines = vec![self.aux_overlay_context_label()];
        lines.extend(slots.iter().enumerate().map(|(index, slot)| {
            let mut parts = Vec::new();
            if let Some(turn) = &slot.turn {
                parts.push(turn.label.clone());
            }
            if let Some(press) = &slot.press {
                parts.push(format!("!{}", press.label));
            }
            let body = if parts.is_empty() {
                "-".into()
            } else {
                parts.join("/")
            };
            format!("A{} {body}", index + 1)
        }));
        Some((title.into(), lines))
    }

    pub(super) fn effective_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        let auto =
            self.resolve_aux_auto_map(&path, selected_key.as_deref(), selected_action.as_ref());
        let custom = self.resolve_custom_aux_slot(index);
        let auto_slot = auto.get(index).and_then(|slot| slot.as_ref());
        let custom_slot = custom.as_ref();
        let turn = custom_slot
            .and_then(|slot| slot.turn.clone())
            .or_else(|| auto_slot.and_then(|slot| slot.turn.clone()));
        let press = custom_slot
            .and_then(|slot| slot.press.clone())
            .or_else(|| auto_slot.and_then(|slot| slot.press.clone()));
        let turn_source = if custom_slot.and_then(|slot| slot.turn.as_ref()).is_some() {
            AuxBindingSource::Custom
        } else if auto_slot.and_then(|slot| slot.turn.as_ref()).is_some() {
            AuxBindingSource::Auto
        } else {
            AuxBindingSource::None
        };
        let press_source = if custom_slot.and_then(|slot| slot.press.as_ref()).is_some() {
            AuxBindingSource::Custom
        } else if auto_slot.and_then(|slot| slot.press.as_ref()).is_some() {
            AuxBindingSource::Auto
        } else {
            AuxBindingSource::None
        };
        ResolvedAuxSlot {
            turn,
            press,
            turn_source,
            press_source,
        }
    }

    pub(super) fn auto_map_prefix_for_line(
        &self,
        key: Option<&str>,
        action: Option<&NativeMenuAction>,
    ) -> Option<String> {
        let path = self.menu.current_focus_path();
        let auto = self.resolve_aux_auto_map(&path, key, action);
        for (index, slot) in auto.iter().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            if let Some(key) = key {
                if slot.turn.as_ref().map(|turn| turn.key.as_str()) == Some(key) {
                    return Some(format!("{}-", index + 1));
                }
            }
            if let Some(action) = action {
                if slot
                    .press
                    .as_ref()
                    .map(|press| self.aux_actions_match(&press.action, action))
                    .unwrap_or(false)
                {
                    return Some(format!("{}!", index + 1));
                }
            }
        }
        None
    }

    fn resolve_custom_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        let binding = self.aux_bindings.get(index)?.as_ref()?;
        let turn = binding.turn_key.as_ref().map(|key| ResolvedAuxTurn {
            key: key.clone(),
            label: self.aux_binding_key_label(key),
        });
        let press = binding
            .press_action
            .as_ref()
            .map(|action| ResolvedAuxPress {
                action: action.clone(),
                label: self.aux_binding_action_label(action),
            });
        Some(ResolvedAuxSlot {
            turn_source: if turn.is_some() {
                AuxBindingSource::Custom
            } else {
                AuxBindingSource::None
            },
            press_source: if press.is_some() {
                AuxBindingSource::Custom
            } else {
                AuxBindingSource::None
            },
            turn,
            press,
        })
    }

    pub(super) fn aux_binding_key_label(&self, key: &str) -> String {
        self.menu
            .binding_spec_for_key(key)
            .and_then(|binding| binding.label)
            .unwrap_or_else(|| match key {
                "masterVolume" => "Master Vol".into(),
                _ => key.rsplit('.').next().unwrap_or(key).into(),
            })
    }

    pub(super) fn aux_binding_action_label(&self, action: &NativeMenuAction) -> String {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => self
                .l1_menu_items()
                .into_iter()
                .find_map(|item| match item.value {
                    NativeMenuValue::Action(NativeMenuAction::BehaviorAction(ref current))
                        if current == action_type =>
                    {
                        Some(item.label)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| action_type.clone()),
            NativeMenuAction::PlatformEffect(action_type) if action_type == "dance.fx.map" => {
                "Map".into()
            }
            NativeMenuAction::PlatformEffect(action_type) if action_type == "midi.panic" => {
                "MIDI Panic".into()
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type == "store.refresh" || action_type == "preset.refresh" =>
            {
                "Refresh".into()
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type.starts_with("sample.assign:") =>
            {
                "Assign".into()
            }
            NativeMenuAction::ResetBehavior => "Reset".into(),
            _ => "Action".into(),
        }
    }

    pub(super) fn aux_actions_match(
        &self,
        left: &NativeMenuAction,
        right: &NativeMenuAction,
    ) -> bool {
        match (left, right) {
            (NativeMenuAction::BehaviorAction(lhs), NativeMenuAction::BehaviorAction(rhs)) => {
                lhs == rhs
            }
            (NativeMenuAction::PlatformEffect(lhs), NativeMenuAction::PlatformEffect(rhs)) => {
                lhs == rhs
            }
            (NativeMenuAction::ResetBehavior, NativeMenuAction::ResetBehavior) => true,
            _ => false,
        }
    }

    fn aux_overlay_context_label(&self) -> String {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        if let Some(key) = selected_key {
            if key.contains(".synth.filter.") {
                return "Synth Filter".into();
            }
            if key.contains(".sample.filter.") {
                return "Sample Filter".into();
            }
            if key.contains(".synth.ampEnv.") || key.contains(".sample.ampEnv.") {
                return "Amp Env".into();
            }
            if key.contains(".synth.filterEnv.") || key.contains(".sample.filterEnv.") {
                return "Filter Env".into();
            }
            if key.contains(".synth.osc1.") {
                return "Osc 1".into();
            }
            if key.contains(".synth.osc2.") {
                return "Osc 2".into();
            }
            if key.contains("mixer.buses.") {
                return "FX Bus".into();
            }
            if key.contains("mixer.master.slots.") {
                return "Global FX".into();
            }
            if key.starts_with("parts.") && key.contains(".behaviorConfig.") {
                return "Life".into();
            }
            if key.contains("dance.fx.params.") {
                return "Dance FX".into();
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
}
