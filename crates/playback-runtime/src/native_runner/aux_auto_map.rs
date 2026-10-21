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

/// Auto-map slots: the plain Aux 1-3 first, then the Shift + Aux 1-3 bank.
pub(super) const AUX_AUTO_MAP_SLOTS: usize = 2 * platform_core::AUX_ENCODER_COUNT;
pub(super) type AuxAutoMap = [Option<ResolvedAuxSlot>; AUX_AUTO_MAP_SLOTS];

/// Lays out slots in priority order, leaving the rest of the six empty.
pub(super) fn aux_auto_map<const N: usize>(slots: [Option<ResolvedAuxSlot>; N]) -> AuxAutoMap {
    let mut map = AuxAutoMap::default();
    for (index, slot) in slots.into_iter().enumerate() {
        map[index] = slot;
    }
    map
}

impl NativeRunner {
    pub(super) fn effective_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        if self.display.ui.shift_held || self.display.ui.combined_modifier_held {
            return self.effective_shift_aux_slot(index);
        }
        let custom = self.resolve_custom_aux_slot(index);
        merge_aux_slots(custom, self.current_auto_aux_slot(index))
    }

    pub(super) fn effective_shift_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        let custom = self.resolve_custom_aux_slot_from(&self.shift_aux_bindings, index);
        merge_aux_slots(
            custom,
            self.current_auto_aux_slot(index + platform_core::AUX_ENCODER_COUNT),
        )
    }

    fn current_auto_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        self.current_auto_aux_map().into_iter().nth(index).flatten()
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
            let encoder = aux_encoder_label(index);
            if let Some(key) = key {
                if slot.turn.as_ref().map(|turn| turn.key.as_str()) == Some(key) {
                    return Some(format!("{encoder}-"));
                }
            }
            if let Some(action) = action {
                if slot
                    .press
                    .as_ref()
                    .map(|press| self.aux_actions_match(&press.action, action))
                    .unwrap_or(false)
                {
                    return Some(format!("{encoder}!"));
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
                .build_menu_items()
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
            NativeMenuAction::PlatformEffect(action_type) if action_type == "play.fx.map" => {
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
    fn current_auto_aux_map(&self) -> AuxAutoMap {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        self.resolve_aux_auto_map(&path, selected_key.as_deref(), selected_action.as_ref())
    }
}

fn aux_encoder_label(index: usize) -> String {
    let count = platform_core::AUX_ENCODER_COUNT;
    if index < count {
        format!("{}", index + 1)
    } else {
        format!("S{}", index - count + 1)
    }
}

fn merge_aux_slots(
    custom: Option<ResolvedAuxSlot>,
    auto: Option<ResolvedAuxSlot>,
) -> ResolvedAuxSlot {
    let custom = custom.as_ref();
    let auto = auto.as_ref();
    ResolvedAuxSlot {
        turn: custom
            .and_then(|slot| slot.turn.clone())
            .or_else(|| auto.and_then(|slot| slot.turn.clone())),
        press: custom
            .and_then(|slot| slot.press.clone())
            .or_else(|| auto.and_then(|slot| slot.press.clone())),
        turn_source: resolved_aux_binding_source(
            custom.and_then(|slot| slot.turn.as_ref()).is_some(),
            auto.and_then(|slot| slot.turn.as_ref()).is_some(),
        ),
        press_source: resolved_aux_binding_source(
            custom.and_then(|slot| slot.press.as_ref()).is_some(),
            auto.and_then(|slot| slot.press.as_ref()).is_some(),
        ),
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
