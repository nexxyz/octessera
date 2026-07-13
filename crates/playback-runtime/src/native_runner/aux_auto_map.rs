use super::*;
use crate::native_menu::NativeMenuValue;

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
    pub(super) fn effective_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        if self.ui.shift_held || self.ui.combined_modifier_held {
            return self.resolve_shift_aux_slot(index);
        }
        let auto = self.current_auto_aux_map();
        let custom = self.resolve_custom_aux_slot(index);
        let auto_slot = auto.get(index).and_then(|slot| slot.as_ref());
        let custom_slot = custom.as_ref();
        ResolvedAuxSlot {
            turn: custom_slot
                .and_then(|slot| slot.turn.clone())
                .or_else(|| auto_slot.and_then(|slot| slot.turn.clone())),
            press: custom_slot
                .and_then(|slot| slot.press.clone())
                .or_else(|| auto_slot.and_then(|slot| slot.press.clone())),
            turn_source: resolved_aux_binding_source(
                custom_slot.and_then(|slot| slot.turn.as_ref()).is_some(),
                auto_slot.and_then(|slot| slot.turn.as_ref()).is_some(),
            ),
            press_source: resolved_aux_binding_source(
                custom_slot.and_then(|slot| slot.press.as_ref()).is_some(),
                auto_slot.and_then(|slot| slot.press.as_ref()).is_some(),
            ),
        }
    }

    fn resolve_shift_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        self.resolve_custom_aux_slot_from(&self.shift_aux_bindings, index)
            .unwrap_or(ResolvedAuxSlot {
                turn: None,
                press: None,
                turn_source: AuxBindingSource::None,
                press_source: AuxBindingSource::None,
            })
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
        self.resolve_custom_aux_slot_from(&self.aux_bindings, index)
    }

    fn resolve_custom_aux_slot_from(
        &self,
        bindings: &[Option<NativeAuxBinding>],
        index: usize,
    ) -> Option<ResolvedAuxSlot> {
        let binding = bindings.get(index)?.as_ref()?;
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
                .worlds_menu_items()
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
            NativeMenuAction::PlatformEffect(action_type) if action_type == "sparks.fx.map" => {
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
    fn current_auto_aux_map(&self) -> Vec<Option<ResolvedAuxSlot>> {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        self.resolve_aux_auto_map(&path, selected_key.as_deref(), selected_action.as_ref())
            .to_vec()
    }
}

fn resolved_aux_binding_source(has_custom: bool, has_auto: bool) -> AuxBindingSource {
    if has_custom {
        AuxBindingSource::Custom
    } else if has_auto {
        AuxBindingSource::Auto
    } else {
        AuxBindingSource::None
    }
}
