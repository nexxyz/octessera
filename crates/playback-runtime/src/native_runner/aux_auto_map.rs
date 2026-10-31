use super::*;
use crate::native_menu::NativeMenuValue;
use crate::oled_frame::OledAuxCap;

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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AuxBindingSource {
    Auto,
    Custom,
    #[default]
    None,
}

#[derive(Clone, Default)]
pub(super) struct ResolvedAuxSlot {
    pub(super) turn: Option<ResolvedAuxTurn>,
    pub(super) press: Option<ResolvedAuxPress>,
    pub(super) turn_source: AuxBindingSource,
    pub(super) press_source: AuxBindingSource,
}

/// Auto-map slots: the plain Aux 1-3 first, then the Shift + Aux 1-3 bank.
pub(super) const AUX_AUTO_MAP_SLOTS: usize = 2 * platform_core::AUX_ENCODER_COUNT;
pub(super) type AuxAutoMap = [Option<ResolvedAuxSlot>; AUX_AUTO_MAP_SLOTS];
/// Manual slots in the same order: the plain Aux 1-3 bindings, then the Shift + Aux 1-3 ones.
pub(super) type AuxManualSlots = [[Option<ResolvedAuxSlot>; platform_core::AUX_ENCODER_COUNT]; 2];

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
        merge_aux_slots(
            custom,
            self.current_auto_aux_slot(index),
            self.aux_auto_first(),
        )
    }

    pub(super) fn effective_shift_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        let custom = self.resolve_custom_aux_slot_from(&self.shift_aux_bindings, index);
        merge_aux_slots(
            custom,
            self.current_auto_aux_slot(index + platform_core::AUX_ENCODER_COUNT),
            self.aux_auto_first(),
        )
    }

    /// What all six Aux sides do with `manual` and `auto` merged under Aux Priority, for the
    /// footer hints and the row caps alike.
    pub(super) fn effective_aux_map(
        &self,
        manual: &AuxManualSlots,
        auto: AuxAutoMap,
    ) -> [ResolvedAuxSlot; AUX_AUTO_MAP_SLOTS] {
        let count = platform_core::AUX_ENCODER_COUNT;
        let auto_first = self.aux_auto_first();
        let mut auto = auto.into_iter();
        std::array::from_fn(|index| {
            let custom = manual[index / count][index % count].clone();
            merge_aux_slots(custom, auto.next().flatten(), auto_first)
        })
    }

    /// Aux Priority Auto: the page's auto map wins a side it fills; Manual: your bindings win.
    fn aux_auto_first(&self) -> bool {
        self.aux_priority == "auto"
    }

    fn current_auto_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        self.current_auto_aux_map().into_iter().nth(index).flatten()
    }

    /// The Aux cap whose turn or click runs a row, with the auto map resolved in that row's own
    /// context, and whether its click (rather than its turn) does.
    pub(super) fn aux_cap_for_line(
        &self,
        manual: &AuxManualSlots,
        key: Option<&str>,
        action: Option<&NativeMenuAction>,
    ) -> Option<(OledAuxCap, bool)> {
        let path = self.menu.current_focus_path();
        let auto = self.resolve_aux_auto_map(&path, key, action);
        let slots = self.effective_aux_map(manual, auto);
        slots.iter().enumerate().find_map(|(index, slot)| {
            let turn = key
                .is_some_and(|key| slot.turn.as_ref().map(|turn| turn.key.as_str()) == Some(key));
            let press = !turn
                && action.is_some_and(|action| {
                    slot.press
                        .as_ref()
                        .is_some_and(|press| self.aux_actions_match(&press.action, action))
                });
            (turn || press).then(|| (aux_cap(index), press))
        })
    }

    fn resolve_custom_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        self.resolve_custom_aux_slot_from(&self.aux_bindings, index)
    }

    pub(super) fn resolve_custom_aux_slot_from(
        &self,
        bindings: &[Option<NativeAuxBinding>],
        index: usize,
    ) -> Option<ResolvedAuxSlot> {
        let binding = bindings.get(index)?.as_ref()?;
        Some(custom_aux_slot(
            binding,
            |key| self.aux_binding_key_label(key),
            |action| self.aux_binding_action_label(action),
        ))
    }

    pub(super) fn aux_binding_key_label(&self, key: &str) -> String {
        self.aux_key_target_label(key)
            .unwrap_or_else(|| key.rsplit('.').next().unwrap_or(key).into())
    }

    /// The label of the row a bound key turns, `None` when no such row exists any more.
    pub(super) fn aux_key_target_label(&self, key: &str) -> Option<String> {
        self.menu
            .binding_spec_for_key(key)
            .and_then(|binding| binding.label)
            .or_else(|| self.hidden_delay_time_item(key).map(|item| item.label))
    }

    pub(super) fn aux_binding_action_label(&self, action: &NativeMenuAction) -> String {
        self.aux_action_target_label(action)
            .unwrap_or_else(|| match action {
                NativeMenuAction::BehaviorAction(id) | NativeMenuAction::PlatformEffect(id) => {
                    id.clone()
                }
                _ => "?".into(),
            })
    }

    /// The short name of a bound click action, `None` when nothing runs it any more.
    pub(super) fn aux_action_target_label(&self, action: &NativeMenuAction) -> Option<String> {
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
                }),
            NativeMenuAction::PlatformEffect(action_type) if action_type == "play.fx.map" => {
                Some("Map".into())
            }
            NativeMenuAction::PlatformEffect(action_type) if action_type == "midi.panic" => {
                Some("MIDI Panic".into())
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type == "store.refresh" || action_type == "preset.refresh" =>
            {
                Some("Refresh".into())
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type.starts_with("sample.assign:") =>
            {
                Some("Assign".into())
            }
            NativeMenuAction::ResetBehavior => Some("Reset".into()),
            _ => self.menu.action_label(action),
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
    pub(super) fn current_auto_aux_map(&self) -> AuxAutoMap {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        self.resolve_aux_auto_map(&path, selected_key.as_deref(), selected_action.as_ref())
    }
}

/// A resolved custom slot for `binding`, its labels from `turn_label` and `press_label`.
pub(super) fn custom_aux_slot(
    binding: &NativeAuxBinding,
    turn_label: impl Fn(&str) -> String,
    press_label: impl Fn(&NativeMenuAction) -> String,
) -> ResolvedAuxSlot {
    let turn = binding.turn_key.as_ref().map(|key| ResolvedAuxTurn {
        key: key.clone(),
        label: turn_label(key),
    });
    let press = binding
        .press_action
        .as_ref()
        .map(|action| ResolvedAuxPress {
            action: action.clone(),
            label: press_label(action),
        });
    ResolvedAuxSlot {
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
    }
}

pub(super) fn aux_cap(index: usize) -> OledAuxCap {
    let count = platform_core::AUX_ENCODER_COUNT;
    OledAuxCap {
        encoder: (index % count + 1) as u8,
        shift: index >= count,
    }
}

/// The one rule for an Aux side with both a manual and an auto target: each turn and each
/// click goes to the first that has one, manual first unless `auto_first`.
fn merge_aux_slots(
    custom: Option<ResolvedAuxSlot>,
    auto: Option<ResolvedAuxSlot>,
    auto_first: bool,
) -> ResolvedAuxSlot {
    let (custom, auto) = (custom.unwrap_or_default(), auto.unwrap_or_default());
    let (turn, turn_source) = pick_aux_side(
        (custom.turn, AuxBindingSource::Custom),
        (auto.turn, AuxBindingSource::Auto),
        auto_first,
    );
    let (press, press_source) = pick_aux_side(
        (custom.press, AuxBindingSource::Custom),
        (auto.press, AuxBindingSource::Auto),
        auto_first,
    );
    ResolvedAuxSlot {
        turn,
        press,
        turn_source,
        press_source,
    }
}

fn pick_aux_side<T>(
    custom: (Option<T>, AuxBindingSource),
    auto: (Option<T>, AuxBindingSource),
    auto_first: bool,
) -> (Option<T>, AuxBindingSource) {
    let (first, second) = if auto_first {
        (auto, custom)
    } else {
        (custom, auto)
    };
    match (first, second) {
        ((Some(target), source), _) | (_, (Some(target), source)) => (Some(target), source),
        _ => (None, AuxBindingSource::None),
    }
}
