use super::*;
use crate::native_menu::PlatformAction;
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
        self.merge_aux_side(custom, self.current_auto_aux_slot(index))
    }

    pub(super) fn effective_shift_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        let custom = self.resolve_custom_aux_slot_from(&self.shift_aux_bindings, index);
        let auto = self.current_auto_aux_slot(index + platform_core::AUX_ENCODER_COUNT);
        self.merge_aux_side(custom, auto)
    }

    /// What all six Aux sides do with `manual` and `auto` merged under Aux Priority, for the
    /// footer hints and the row caps alike.
    pub(super) fn effective_aux_map(
        &self,
        manual: &AuxManualSlots,
        auto: AuxAutoMap,
    ) -> [ResolvedAuxSlot; AUX_AUTO_MAP_SLOTS] {
        let count = platform_core::AUX_ENCODER_COUNT;
        let mut auto = auto.into_iter();
        std::array::from_fn(|index| {
            let custom = manual[index / count][index % count].clone();
            self.merge_aux_side(custom, auto.next().flatten())
        })
    }

    /// One side's manual and auto slots merged under Aux Priority: Auto lets the page's auto
    /// map win a side it fills, Manual lets your bindings win. While the performance view shows
    /// its Aux visual, the bank picked there applies strictly instead.
    fn merge_aux_side(
        &self,
        custom: Option<ResolvedAuxSlot>,
        auto: Option<ResolvedAuxSlot>,
    ) -> ResolvedAuxSlot {
        match self.aux_view_bank() {
            Some(AuxPriority::Manual) => merge_aux_slots(custom, None, false),
            Some(AuxPriority::Auto) => merge_aux_slots(None, auto, true),
            None => merge_aux_slots(custom, auto, self.aux_priority == AuxPriority::Auto),
        }
    }

    fn current_auto_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        self.current_auto_aux_map().into_iter().nth(index).flatten()
    }

    /// The Aux cap whose turn or click runs a row, with the auto map resolved in that row's own
    /// context, and whether its click (rather than its turn) does.
    pub(super) fn aux_cap_for_line(
        &self,
        manual: &AuxManualSlots,
        focus_path: &str,
        key: Option<&str>,
        action: Option<&NativeMenuAction>,
    ) -> Option<(OledAuxCap, bool)> {
        let auto = self.resolve_aux_auto_map(focus_path, key, action);
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

    /// A turn target's label with the short name of what it belongs to (`I1 Cutoff`, `B3 Mix`,
    /// `L5 Density`, `M Drive`), as the footer hints and the mappings modal show it.
    pub(super) fn aux_turn_label_with_target(turn: &ResolvedAuxTurn) -> String {
        match aux_target_shorthand(&turn.key) {
            Some(shorthand) if !turn.label.is_empty() => format!("{shorthand} {}", turn.label),
            _ => turn.label.clone(),
        }
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
                NativeMenuAction::BehaviorAction(id) => id.clone(),
                NativeMenuAction::PlatformEffect(action) => action.as_wire(),
                NativeMenuAction::LayerBehaviorAction {
                    layer_index,
                    action_type,
                } => format!(
                    "L{} {}",
                    layer_index + 1,
                    super::bound_behavior_click::behavior_action_name(action_type)
                        .unwrap_or_else(|| action_type.clone())
                ),
                _ => "?".into(),
            })
    }

    /// The short name of a bound click action, `None` when nothing runs it any more.
    pub(super) fn aux_action_target_label(&self, action: &NativeMenuAction) -> Option<String> {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => {
                self.layer_behavior_action_label(self.active_layer_index, action_type)
            }
            NativeMenuAction::LayerBehaviorAction {
                layer_index,
                action_type,
            } => self
                .layer_behavior_action_label(*layer_index, action_type)
                .map(|label| format!("L{} {label}", layer_index + 1)),
            NativeMenuAction::PlatformEffect(PlatformAction::PlayFxMap) => Some("Map".into()),
            NativeMenuAction::PlatformEffect(PlatformAction::MidiPanic) => {
                Some("MIDI Panic".into())
            }
            NativeMenuAction::PlatformEffect(PlatformAction::PresetRefresh) => {
                Some("Refresh".into())
            }
            NativeMenuAction::PlatformEffect(PlatformAction::SampleAssign { .. }) => {
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
            (
                NativeMenuAction::BehaviorAction(_) | NativeMenuAction::LayerBehaviorAction { .. },
                NativeMenuAction::BehaviorAction(_) | NativeMenuAction::LayerBehaviorAction { .. },
            ) => self.behavior_click(left) == self.behavior_click(right),
            (NativeMenuAction::PlatformEffect(lhs), NativeMenuAction::PlatformEffect(rhs)) => {
                lhs == rhs
            }
            (NativeMenuAction::ResetBehavior, NativeMenuAction::ResetBehavior) => true,
            _ => false,
        }
    }
    pub(super) fn current_auto_aux_map(&self) -> AuxAutoMap {
        if self.shows_performance_view() {
            return self.performance_view_auto_aux_map();
        }
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let selected_key = selected_key.or_else(|| {
            self.menu
                .current_key()
                .filter(|key| *key == "behaviorId")
                .map(str::to_owned)
        });
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

/// The short name of the instrument, bus, layer or master chain a key belongs to.
fn aux_target_shorthand(key: &str) -> Option<String> {
    let mut parts = key.split('.');
    let numbered = |prefix: &str, index: Option<&str>| {
        index
            .and_then(|index| index.parse::<usize>().ok())
            .map(|index| format!("{prefix}{}", index + 1))
    };
    match (parts.next(), parts.next()) {
        (Some("instruments"), index) => numbered("I", index),
        (Some("layers"), index) => numbered("L", index),
        (Some("mixer"), Some("buses")) => numbered("B", parts.next()),
        (Some("mixer"), Some("master")) => Some("M".into()),
        _ => None,
    }
}

#[cfg(test)]
#[test]
fn a_turn_target_names_the_instrument_bus_layer_or_master_chain_it_belongs_to() {
    let shorthand = |key: &str| aux_target_shorthand(key);
    assert_eq!(
        shorthand("instruments.0.synth.filter.cutoffHz").as_deref(),
        Some("I1")
    );
    assert_eq!(
        shorthand("mixer.buses.2.slot1.params.mixPct").as_deref(),
        Some("B3")
    );
    assert_eq!(
        shorthand("layers.4.build.behaviorConfig.density").as_deref(),
        Some("L5")
    );
    assert_eq!(
        shorthand("mixer.master.slots.0.params.drivePct").as_deref(),
        Some("M")
    );
    assert_eq!(shorthand("masterVolume"), None);
    assert_eq!(shorthand("instruments.x.synth"), None);
}
