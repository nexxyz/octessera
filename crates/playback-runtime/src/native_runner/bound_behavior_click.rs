//! Behavior actions bound to an Aux click or a Faders action tile: each keeps the layer it was
//! bound on and runs there whichever layer is active.
use super::*;
use crate::native_menu::NativeMenuValue;
use crate::protocol::RunnerMessage;

impl NativeRunner {
    /// The Build label of `action_type` on the current behavior of `layer_index`, `None` when
    /// that behavior has no such action.
    pub(super) fn layer_behavior_action_label(
        &self,
        layer_index: usize,
        action_type: &str,
    ) -> Option<String> {
        let items = if layer_index == self.active_layer_index {
            self.build_menu_items()
        } else {
            self.behavior_target_items_for_layer(layer_index)
        };
        items.into_iter().find_map(|item| match item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(ref current))
                if current == action_type =>
            {
                Some(item.label)
            }
            _ => None,
        })
    }

    /// A click as it is bound: a Build row's behavior action keeps the layer it was bound on.
    pub(super) fn bound_click_action(&self, action: NativeMenuAction) -> NativeMenuAction {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => {
                NativeMenuAction::LayerBehaviorAction {
                    layer_index: self.active_layer_index,
                    action_type,
                }
            }
            other => other,
        }
    }

    /// Runs a bound click, toasting `head: label`: a behavior action on its layer, so its notes
    /// reach Link and audio, anything else as its menu action.
    pub(super) fn run_bound_click(
        &mut self,
        action: NativeMenuAction,
        head: &str,
        label: &str,
    ) -> Result<Vec<RunnerMessage>, String> {
        if !self.bound_action_active(&action) {
            self.show_toast(format!("{head}: {label} not active"));
            return self.messages_with_snapshot();
        }
        let toast = format!("{head}: {label}");
        if let Some((layer_index, action_type)) = self.behavior_click(&action) {
            let quiet = self.quiet_behavior_click(&action);
            let result = self.trigger_layer_behavior_action_result(layer_index, action_type)?;
            if !quiet {
                self.show_toast(toast);
            }
            return self.messages_with_layer_input_result(layer_index, result);
        }
        let effect = self.execute_menu_action(action)?;
        self.show_toast(toast);
        match effect {
            Some(effect) => self.messages_with_effects(vec![effect]),
            None => self.messages_with_snapshot(),
        }
    }

    /// A looper punch toasts its own mode, so its click toast stays quiet.
    pub(super) fn quiet_behavior_click(&self, action: &NativeMenuAction) -> bool {
        self.behavior_click(action)
            .is_some_and(|(layer_index, action_type)| {
                action_type == "toggleMode" && self.layer_behavior_id(layer_index) == "looper"
            })
    }

    /// The layer and action type of a behavior click, bound or auto-mapped.
    pub(super) fn behavior_click(&self, action: &NativeMenuAction) -> Option<(usize, String)> {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => {
                Some((self.active_layer_index, action_type.clone()))
            }
            NativeMenuAction::LayerBehaviorAction {
                layer_index,
                action_type,
            } => Some((*layer_index, action_type.clone())),
            _ => None,
        }
    }
}

/// The Build label some behavior gives `action_type`, for a bound click whose layer no longer
/// offers it.
pub(super) fn behavior_action_name(action_type: &str) -> Option<String> {
    platform_core::behavior_catalog().iter().find_map(|entry| {
        let behavior = platform_core::get_native_behavior(entry.id)?;
        let state = behavior.init(Value::Null).ok()?;
        behavior
            .config_menu(&state)
            .ok()??
            .into_iter()
            .find(|item| {
                item.item_type == platform_core::BehaviorConfigItemType::Action
                    && item.key == action_type
            })
            .map(|item| item.label)
    })
}
