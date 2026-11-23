//! A change to a layer's Link, from the menu or a binding, lands here with the Link as it was
//! before the edit. A change to where the layer's presses and releases go (a row's instrument or
//! action, Note Preview, Scan Mode, Axis, Sections or Lanes, the Arp turned on or off, the gate
//! closing) drains the instruments the old rows played; a Scan Dir flip turns the line round;
//! an Arp Mode or Source change lets go of the keys the Arp holds.

use super::{
    link_layer_config, ArpMode, NativeLinkLayer, NativeRunner, ScanAxis, ScanMode, TriggerAction,
    TriggerProbabilityMode,
};
use platform_core::{NativeBehavior, PlayStyle};

impl NativeRunner {
    /// Follows a change of layer `index`'s Link from `before`.
    pub(super) fn apply_link_change(&mut self, index: usize, before: &NativeLinkLayer) {
        let Some(layer) = self.layers.get_mut(index) else {
            return;
        };
        let after = &layer.link;
        let retargets = routes_differently(before, after);
        let arp_keys_change =
            (before.arp.mode, before.arp.source) != (after.arp.mode, after.arp.source);
        if before.scan_direction != after.scan_direction {
            layer.engine.turn_scan_line_round();
        }
        if retargets {
            self.drain_layer_notes(index, target_slots(before));
        } else if arp_keys_change {
            self.clear_link_arp_state_for_layer(index);
        }
    }
}

impl NativeRunner {
    /// A layer that picks `behavior` takes its play style's Scan Mode and Note Preview: a grid
    /// sequencer scans with a line moving left to right and previews presses, a generative
    /// world plays directly and previews them, immediate play and games play directly and leave
    /// a press to the behavior. Both stay editable until the next behavior pick; a patch load
    /// keeps the saved values.
    pub(super) fn apply_behavior_link_defaults(&mut self, index: usize, behavior: NativeBehavior) {
        let Some(link) = self.layers.get_mut(index).map(|layer| &mut layer.link) else {
            return;
        };
        let before = link.clone();
        let style = behavior.play_style();
        link.scan_mode = if style == PlayStyle::GridSequencer {
            link.scan_axis = ScanAxis::Columns;
            ScanMode::Scanning
        } else {
            ScanMode::None
        };
        link.note_preview = matches!(style, PlayStyle::GridSequencer | PlayStyle::Generative);
        if *link == before {
            return;
        }
        let sense = link_layer_config(link);
        self.apply_link_change(index, &before);
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

/// Whether the edit moves where the layer's presses and releases go.
fn routes_differently(before: &NativeLinkLayer, after: &NativeLinkLayer) -> bool {
    let rows = |link: &NativeLinkLayer| {
        [
            (link.activate_slot, link.activate_action),
            (link.stable_slot, link.stable_action),
            (link.deactivate_slot, link.deactivate_action),
        ]
    };
    let lanes = |link: &NativeLinkLayer| {
        (
            link.note_preview,
            link.scan_mode,
            link.scan_axis,
            link.scan_sections,
            link.scan_lanes,
            link.arp.mode == ArpMode::None,
        )
    };
    let closes = before.trigger_probability_mode != TriggerProbabilityMode::Zero
        && after.trigger_probability_mode == TriggerProbabilityMode::Zero;
    rows(before) != rows(after) || lanes(before) != lanes(after) || closes
}

/// The instrument slots `link`'s On, Hold and Off rows play notes on, as a bit mask.
pub(super) fn target_slots(link: &NativeLinkLayer) -> u16 {
    [
        (link.activate_slot, link.activate_action),
        (link.stable_slot, link.stable_action),
        (link.deactivate_slot, link.deactivate_action),
    ]
    .into_iter()
    .filter(|(_, action)| *action == TriggerAction::NoteOn)
    .fold(0, |slots, (slot, _)| {
        slots
            | u32::try_from(slot)
                .ok()
                .and_then(|slot| 1_u16.checked_shl(slot))
                .unwrap_or(0)
    })
}
