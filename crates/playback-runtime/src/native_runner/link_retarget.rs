//! A change to a layer's Link, from the menu or a binding, lands here with the Link's routing as
//! it was before the edit. A change to where the layer's presses and releases go (a row's
//! instrument or action, Note Preview, Scan Mode, Axis, Sections or Lanes, the Arp turned on or
//! off, the gate closing) drains the instruments the old rows played; a Scan Dir flip turns the
//! line round; an Arp Mode or Source change lets go of the keys the Arp holds.

use super::{
    link_layer_config, ArpMode, ArpSource, NativeLinkLayer, NativeRunner, ScanAxis, ScanDirection,
    ScanLanes, ScanMode, TriggerAction, TriggerProbabilityMode,
};
use platform_core::{NativeBehavior, PlayStyle};

/// What of a layer's Link decides where its presses and releases go; cheap to copy before an
/// edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct LinkRouting {
    rows: [(usize, TriggerAction); 3],
    note_preview: bool,
    scan_mode: ScanMode,
    scan_axis: ScanAxis,
    scan_direction: ScanDirection,
    scan_sections: u8,
    scan_lanes: ScanLanes,
    arp: (ArpMode, ArpSource),
    gate_closed: bool,
}

impl LinkRouting {
    pub(super) fn of(link: &NativeLinkLayer) -> Self {
        Self {
            rows: [
                (link.activate_slot, link.activate_action),
                (link.stable_slot, link.stable_action),
                (link.deactivate_slot, link.deactivate_action),
            ],
            note_preview: link.note_preview,
            scan_mode: link.scan_mode,
            scan_axis: link.scan_axis,
            scan_direction: link.scan_direction,
            scan_sections: link.scan_sections,
            scan_lanes: link.scan_lanes,
            arp: (link.arp.mode, link.arp.source),
            gate_closed: link.trigger_probability_mode == TriggerProbabilityMode::Zero,
        }
    }

    /// Whether going to `after` moves where the layer's presses and releases go.
    fn retargets(&self, after: &Self) -> bool {
        let lanes = |routing: &Self| {
            (
                routing.note_preview,
                routing.scan_mode,
                routing.scan_axis,
                routing.scan_sections,
                routing.scan_lanes,
                routing.arp.0 == ArpMode::None,
            )
        };
        self.rows != after.rows
            || lanes(self) != lanes(after)
            || !self.gate_closed && after.gate_closed
    }

    /// The instrument slots the On, Hold and Off rows play notes on, as a bit mask.
    fn target_slots(&self) -> u16 {
        self.rows
            .iter()
            .filter(|(_, action)| *action == TriggerAction::NoteOn)
            .fold(0, |slots, (slot, _)| {
                slots
                    | u32::try_from(*slot)
                        .ok()
                        .and_then(|slot| 1_u16.checked_shl(slot))
                        .unwrap_or(0)
            })
    }
}

impl NativeRunner {
    /// Follows a change of layer `index`'s Link from `before`.
    pub(super) fn apply_link_change(&mut self, index: usize, before: LinkRouting) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        let after = LinkRouting::of(&layer.link);
        if before.scan_direction != after.scan_direction {
            layer.engine.turn_scan_line_round();
        }
        if before.retargets(&after) {
            self.drain_layer_notes(index, before.target_slots());
        } else if before.arp != after.arp {
            self.clear_link_arp_state_for_layer(index);
        }
    }

    /// A layer that picks `behavior` takes its play style's scan setup and Note Preview: a grid
    /// sequencer scans with a line moving left to right and previews presses, a generative world
    /// plays directly and previews them, immediate play and games play directly and leave a press
    /// to the behavior; Scan Dir starts forward. All stay editable until the next behavior pick;
    /// a patch load keeps the saved values.
    pub(super) fn apply_behavior_link_defaults(&mut self, index: usize, behavior: NativeBehavior) {
        let Some(link) = self.layers.get_mut(index).map(|layer| &mut layer.link) else {
            return;
        };
        let before = LinkRouting::of(link);
        let style = behavior.play_style();
        link.scan_mode = if style == PlayStyle::GridSequencer {
            link.scan_axis = ScanAxis::Columns;
            ScanMode::Scanning
        } else {
            ScanMode::None
        };
        link.scan_direction = ScanDirection::Forward;
        link.note_preview = matches!(style, PlayStyle::GridSequencer | PlayStyle::Generative);
        if LinkRouting::of(link) == before {
            return;
        }
        let sense = link_layer_config(link);
        self.apply_link_change(index, before);
        self.menu.show_link_scanning_rows(index, &sense);
        self.menu.set_bool_value_for_key(
            &format!("layers.{index}.link.notePreview"),
            sense.note_preview,
        );
        if index == self.active_layer_index {
            self.refresh_active_mapping_config();
            self.refresh_active_interpretation_profile();
        }
    }
}

/// The instrument slots `link`'s On, Hold and Off rows play notes on, as a bit mask.
pub(super) fn target_slots(link: &NativeLinkLayer) -> u16 {
    LinkRouting::of(link).target_slots()
}
