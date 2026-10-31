use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::aux_auto_map::{custom_aux_slot, merge_aux_slots, AuxBindingSource, ResolvedAuxSlot};
use super::{NativeAuxBinding, NativeMenuAction, NativeRunner};
use crate::native_menu::section_labels::{key_home_section_color, section_color, BUILD_LABEL};
use crate::oled_frame::{OledAuxHint, OledAuxHints, OledHintLabel, OledHintSource};

const AUX_COUNT: usize = platform_core::AUX_ENCODER_COUNT;

/// Manual Aux slots with their labels, kept for the bindings they were resolved from: the
/// labels need menu lookups that are too slow for every display capture. A binding whose
/// target no longer exists keeps its side with an empty label, so it shows no hint and still
/// hides the auto hint it overrides.
#[derive(Clone)]
pub(super) struct ManualAuxHintSlots {
    bindings: [Vec<Option<NativeAuxBinding>>; 2],
    audio_config_revision: u64,
    target_context: u64,
    slots: [[Option<ResolvedAuxSlot>; AUX_COUNT]; 2],
}

impl NativeRunner {
    /// What each Aux encoder does on the focused page, plain and Shift layer, for the footer.
    pub(super) fn aux_hints(&self) -> OledAuxHints {
        let ui = &self.display.ui;
        if !ui.aux_auto_hints && !ui.aux_manual_hints {
            return OledAuxHints::default();
        }
        let auto = self.current_auto_aux_map();
        let manual = self.manual_aux_hint_slots();
        let hint = |layer: usize, index: usize| {
            let slot = merge_aux_slots(
                manual[layer][index].clone(),
                auto[layer * AUX_COUNT + index].clone(),
            );
            OledAuxHint {
                turn: slot.turn.as_ref().and_then(|turn| {
                    let color = key_home_section_color(&turn.key);
                    self.hint_label(&turn.label, color, slot.turn_source)
                }),
                click: slot.press.as_ref().and_then(|press| {
                    let color = action_home_color(&press.action);
                    self.hint_label(&press.label, color, slot.press_source)
                }),
            }
        };
        OledAuxHints {
            normal: std::array::from_fn(|index| hint(0, index)),
            shift: std::array::from_fn(|index| hint(1, index)),
        }
    }

    fn hint_label(
        &self,
        label: &str,
        color: u16,
        source: AuxBindingSource,
    ) -> Option<OledHintLabel> {
        let ui = &self.display.ui;
        let source = match source {
            AuxBindingSource::Auto if ui.aux_auto_hints => OledHintSource::Auto,
            AuxBindingSource::Custom if ui.aux_manual_hints => OledHintSource::Manual,
            _ => return None,
        };
        if label.is_empty() {
            return None;
        }
        Some(OledHintLabel {
            text: label.to_string(),
            color,
            source,
        })
    }

    fn manual_aux_hint_slots(&self) -> [[Option<ResolvedAuxSlot>; AUX_COUNT]; 2] {
        let target_context = self.aux_target_context();
        let current = |cached: &ManualAuxHintSlots| {
            cached.bindings[0] == self.aux_bindings
                && cached.bindings[1] == self.shift_aux_bindings
                && cached.audio_config_revision == self.audio_config_revision
                && cached.target_context == target_context
        };
        if let Some(cached) = self.display.manual_aux_hints.borrow().as_ref() {
            if current(cached) {
                return cached.slots.clone();
            }
        }
        let resolved = ManualAuxHintSlots {
            bindings: [self.aux_bindings.clone(), self.shift_aux_bindings.clone()],
            audio_config_revision: self.audio_config_revision,
            target_context,
            slots: [&self.aux_bindings, &self.shift_aux_bindings].map(|bindings| {
                std::array::from_fn(|index| {
                    let binding = bindings.get(index)?.as_ref()?;
                    Some(custom_aux_slot(
                        binding,
                        |key| self.aux_key_target_label(key).unwrap_or_default(),
                        |action| self.aux_action_target_label(action).unwrap_or_default(),
                    ))
                })
            }),
        };
        let slots = resolved.slots.clone();
        *self.display.manual_aux_hints.borrow_mut() = Some(resolved);
        slots
    }

    /// What decides which bound targets exist: the behavior, the FX slot types and the
    /// instrument kinds, hashed so the cache check stays cheap on every capture.
    fn aux_target_context(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.behavior.id().hash(&mut hasher);
        for bus in &self.fx_buses {
            (&bus.slot1_type, &bus.slot2_type, &bus.slot3_type).hash(&mut hasher);
        }
        self.global_fx_slots.hash(&mut hasher);
        for instrument in &self.instruments {
            instrument.kind.hash(&mut hasher);
        }
        hasher.finish()
    }
}

/// The colour of the menu section that owns an Aux click action.
fn action_home_color(action: &NativeMenuAction) -> u16 {
    match action {
        NativeMenuAction::BehaviorAction(_) | NativeMenuAction::ResetBehavior => {
            section_color(BUILD_LABEL)
        }
        NativeMenuAction::PlatformEffect(effect) => key_home_section_color(effect),
        _ => key_home_section_color(""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_runner::NativeRunnerConfig;
    use platform_core::palette;

    fn runner() -> NativeRunner {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        runner.fx_buses[0].slot1_type = "delay".into();
        runner.fx_buses[0].slot1_params = serde_json::json!({ "timeMs": 250, "feedback": 0.35 });
        runner.menu.rebuild(runner.menu_config());
        runner
    }

    #[test]
    fn auto_hints_follow_the_focused_page_and_target_home() {
        let mut runner = runner();
        assert!(runner
            .menu
            .focus_item_key("mixer.buses.0.slot1.params.feedback"));
        let hints = runner.aux_hints();
        let turn = |index: usize| hints.normal[index].turn.as_ref().unwrap();
        assert_eq!(turn(0).text, "FB");
        assert_eq!(turn(1).text, "Mix");
        assert_eq!(turn(2).text, "Time");
        assert!(hints.normal.iter().all(|hint| hint.click.is_none()));
        assert_eq!(turn(0).color, palette::BLUE_RGB565);
        assert_eq!(turn(0).source, OledHintSource::Auto);
        assert_eq!(hints.shift[0].turn.as_ref().unwrap().text, "Spread");

        runner.display.ui.aux_auto_hints = false;
        assert_eq!(runner.aux_hints(), OledAuxHints::default());
    }

    #[test]
    fn manual_hints_win_their_slot_and_follow_their_toggle() {
        let mut runner = runner();
        assert!(runner
            .menu
            .focus_item_key("mixer.buses.0.slot1.params.feedback"));
        runner.aux_bindings[2] = Some(NativeAuxBinding {
            turn_key: Some("masterVolume".into()),
            press_action: Some(NativeMenuAction::PlatformEffect("recording.stop".into())),
        });
        let hints = runner.aux_hints();
        let manual = &hints.normal[2];
        let turn = manual.turn.as_ref().unwrap();
        assert_eq!(turn.source, OledHintSource::Manual);
        assert_eq!(turn.color, palette::GRAY_RGB565);
        assert_eq!(turn.text, "Master Vol");
        let click = manual.click.as_ref().unwrap();
        assert_eq!(click.text, "Stop");
        assert_eq!(click.color, palette::GRAY_RGB565);

        runner.display.ui.aux_manual_hints = false;
        let hints = runner.aux_hints();
        assert!(hints.normal[2].turn.is_none());
        assert!(hints.normal[2].click.is_none());
        assert_eq!(hints.normal[0].turn.as_ref().unwrap().text, "FB");

        runner.display.ui.aux_manual_hints = true;
        runner.aux_bindings[2] = None;
        let hints = runner.aux_hints();
        assert_eq!(hints.normal[2].turn.as_ref().unwrap().text, "Time");
    }

    #[test]
    fn a_manual_hint_whose_target_left_with_its_fx_type_shows_nothing() {
        let mut runner = runner();
        runner.aux_bindings[1] = Some(NativeAuxBinding {
            turn_key: Some("mixer.buses.0.slot1.params.spreadPct".into()),
            press_action: None,
        });
        assert!(runner
            .menu
            .focus_item_key("mixer.buses.0.slot1.params.feedback"));
        assert_eq!(
            runner.aux_hints().normal[1].turn.as_ref().unwrap().text,
            "Spread %"
        );

        runner.edit_menu_key("mixer.buses.0.slot1.type", 1).unwrap();
        assert_ne!(runner.fx_buses[0].slot1_type, "delay");
        assert!(runner
            .menu
            .binding_spec_for_key("mixer.buses.0.slot1.params.spreadPct")
            .is_none());
        assert!(runner.aux_hints().normal[1].turn.is_none());
    }

    #[test]
    fn life_shift_hint_names_the_glider_interval_by_its_distinguishing_word() {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        assert_eq!(runner.behavior.id(), "life");
        assert!(runner
            .menu
            .focus_item_key("layers.0.build.behaviorConfig.randomCellsPerTick"));
        let hints = runner.aux_hints();
        let shift = &hints.shift[0];
        assert_eq!(shift.turn.as_ref().unwrap().text, "Interval");
        assert_eq!(shift.click.as_ref().unwrap().text, "Glider");
        assert_eq!(hints.normal[1].turn.as_ref().unwrap().text, "Count");
    }
}
