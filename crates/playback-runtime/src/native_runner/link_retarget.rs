//! A change to a layer's Link, from the menu or a binding, lands here with the Link as it was
//! before the edit. A change to where the layer's presses and releases go (a row's instrument or
//! action, Note Preview, Scan Mode, Axis, Sections or Lanes, the Arp turned on or off, the gate
//! closing) drains the instruments the old rows played; a Scan Dir flip turns the line round;
//! an Arp Mode or Source change lets go of the keys the Arp holds.

use super::{ArpMode, NativeLinkLayer, NativeRunner, TriggerAction, TriggerProbabilityMode};

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
