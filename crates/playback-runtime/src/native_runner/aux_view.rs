//! The performance view's Aux visual: what each Aux encoder turns and clicks, with the turn
//! target's current value, resolved like the footer hints but whatever the hint settings say.

use super::aux_auto_map::{AuxBindingSource, ResolvedAuxSlot};
use super::aux_hints::action_home_color;
use super::construction_deferred::AUX_TURN_TOAST_HOLD;
use super::performance_view::ViewVisual;
use super::{AuxPriority, NativeRunner};
use crate::native_menu::format_values::format_display_value;
use crate::native_menu::item_gauge;
use crate::native_menu::section_labels::key_home_section_color;
use crate::oled_frame::{OledAuxCard, OledAuxGauge, OledAuxTurn, OledAuxView, OledBarStyle};
use std::time::Instant;

mod compact_path;

const AUX_COUNT: usize = platform_core::AUX_ENCODER_COUNT;

fn bank_label(bank: AuxPriority) -> &'static str {
    match bank {
        AuxPriority::Manual => "Manual",
        AuxPriority::Auto => "Auto",
    }
}

/// The Aux turn the Aux visual highlights in place of its toast.
#[derive(Clone, Copy, Debug)]
pub(super) struct AuxViewTurn {
    index: usize,
    shifted: bool,
    pub(super) until: Instant,
}

impl NativeRunner {
    pub(super) fn shows_aux_view(&self) -> bool {
        self.shows_performance_view() && self.display.session_visual == Some(ViewVisual::Aux)
    }

    /// The bank the Aux visual applies strictly while it shows: the one a Main turn picked this
    /// session, Auto until then. `None` elsewhere, where Aux Priority merges both as usual.
    pub(super) fn aux_view_bank(&self) -> Option<AuxPriority> {
        self.shows_aux_view()
            .then(|| self.display.aux_view_bank.unwrap_or(AuxPriority::Auto))
    }

    /// A Main turn in the Aux visual: right picks the Manual bank, left the Auto bank.
    pub(super) fn turn_aux_view_bank(&mut self, delta: i8) {
        let bank = if delta > 0 {
            AuxPriority::Manual
        } else {
            AuxPriority::Auto
        };
        self.display.aux_view_bank = Some(bank);
        self.show_toast(format!("Aux: {}", bank_label(bank)));
    }

    /// The Aux cards a capture shows, for the layer Shift selects, while nothing covers the view.
    pub(super) fn aux_view_presentation(
        &self,
        menu_frame: bool,
        now: Instant,
    ) -> Option<OledAuxView> {
        if !menu_frame || !self.shows_aux_view() || !self.presents_full_screen() {
            return None;
        }
        let ui = &self.display.ui;
        let shifted = ui.shift_held || ui.combined_modifier_held;
        let slots = self.effective_aux_map(&self.manual_aux_slots(), self.current_auto_aux_map());
        let turned = self
            .display
            .aux_view_turn
            .filter(|turn| now < turn.until && turn.shifted == shifted)
            .map(|turn| turn.index);
        let layer = usize::from(shifted) * AUX_COUNT;
        Some(OledAuxView {
            bank: match self.aux_view_bank() {
                Some(AuxPriority::Manual) => "Man",
                _ => "Auto",
            }
            .into(),
            cards: std::array::from_fn(|index| {
                self.aux_card(&slots[layer + index], turned == Some(index))
            }),
        })
    }

    /// Lights the turned card for as long as the Aux turn toast it replaces would show.
    pub(super) fn mark_aux_view_turn(&mut self, index: usize, shifted: bool) {
        self.display.aux_view_turn = Some(AuxViewTurn {
            index,
            shifted,
            until: self.display.transients.now() + AUX_TURN_TOAST_HOLD,
        });
    }

    fn aux_card(&self, slot: &ResolvedAuxSlot, turning: bool) -> OledAuxCard {
        let turn = slot.turn.as_ref().map(|turn| OledAuxTurn {
            label: if turn.label.is_empty() {
                self.aux_binding_key_label(&turn.key)
            } else {
                turn.label.clone()
            },
            path: (slot.turn_source == AuxBindingSource::Custom)
                .then(|| self.aux_compact_path(&turn.key))
                .flatten(),
            gauge: self.aux_gauge(&turn.key),
        });
        let color = slot
            .turn
            .as_ref()
            .map(|turn| key_home_section_color(&turn.key))
            .or_else(|| {
                slot.press
                    .as_ref()
                    .map(|press| action_home_color(&press.action))
            })
            .unwrap_or_default();
        OledAuxCard {
            turn,
            click: slot.press.as_ref().map(|press| press.label.clone()),
            color,
            turning,
        }
    }

    /// The target's value as its row shows it, and its place on the arc; `None` while no row
    /// holds the target.
    fn aux_gauge(&self, key: &str) -> Option<OledAuxGauge> {
        let item = self
            .generated_behavior_target_item(key)
            .or_else(|| self.menu.item_for_key(key))
            .or_else(|| self.hidden_delay_time_item(key))?;
        let (value, fraction, marker) = item_gauge(&item)?;
        Some(OledAuxGauge {
            value: format_display_value(Some(key), value),
            fraction,
            style: if marker {
                OledBarStyle::Marker
            } else {
                OledBarStyle::Fill
            },
        })
    }

    pub(super) fn aux_compact_path(&self, key: &str) -> Option<String> {
        let behavior_of = |layer: usize| {
            self.layers
                .get(layer)
                .map(|layer| layer.behavior_id.clone())
        };
        match self.menu.item_chain_for_key(key) {
            Some(chain) => compact_path::compact_path(&chain, behavior_of),
            None => {
                let layer =
                    super::behavior_target_menu::parse_generated_behavior_target_layer_index(key)?;
                Some(format!("L{}/{}", layer + 1, behavior_of(layer)?))
            }
        }
    }
}
