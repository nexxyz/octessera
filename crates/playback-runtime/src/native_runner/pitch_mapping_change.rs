//! A Link change that moves which pitch a tile plays (Root, Set, 1st Note, Low/High Note, Pitch
//! Steps, Range, or a modulation of one) releases every key the instruments of the layer's
//! note-on rows hold, at the layer's next step. A change that leaves every pitch where it was, such
//! as an LFO tick that lands on the same Root, releases nothing.

use super::{NativeLinkLayer, NativeRunner, RangeMode, TriggerAction};

#[derive(Clone, Copy, Debug, PartialEq)]
struct PitchRange {
    lowest_note: u8,
    highest_note: u8,
    starting_note: u8,
    column_steps: i32,
    row_steps: i32,
    out_of_range: RangeMode,
}

impl PitchRange {
    fn of(link: &NativeLinkLayer) -> Self {
        Self {
            lowest_note: link.lowest_note.min(link.highest_note),
            highest_note: link.lowest_note.max(link.highest_note),
            starting_note: link.starting_note,
            column_steps: if link.x_pitch_enabled {
                link.x_pitch_steps
            } else {
                0
            },
            row_steps: if link.y_pitch_enabled {
                link.y_pitch_steps
            } else {
                0
            },
            out_of_range: link.out_of_range,
        }
    }
}

/// The Link settings a layer's pitches follow, as it last played them.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PitchMapping {
    range: PitchRange,
    scale: String,
    root: String,
}

impl NativeRunner {
    /// The slots whose keys `layer_index` releases because its pitch mapping moved since it last
    /// played; None when it did not, or when no row plays a note.
    pub(super) fn changed_pitch_mapping_targets(&mut self, layer_index: usize) -> Option<u16> {
        let layer = self.layers.get(layer_index)?;
        let (link, range) = (&layer.link, PitchRange::of(&layer.link));
        let moved = match &layer.pitch_mapping {
            Some(mapping)
                if mapping.range == range
                    && mapping.scale == link.scale
                    && mapping.root == link.root =>
            {
                return None
            }
            Some(mapping) => {
                mapping.range != range
                    || platform_core::expand_note_set(&mapping.scale, &mapping.root)
                        != platform_core::expand_note_set(&link.scale, &link.root)
            }
            None => false,
        };
        let next = PitchMapping {
            range,
            scale: link.scale.clone(),
            root: link.root.clone(),
        };
        self.layers[layer_index].pitch_mapping = Some(next);
        moved
            .then(|| self.layer_target_slots(layer_index))
            .filter(|slots| *slots != 0)
    }

    /// The instrument slots the layer's On, Hold and Off rows play notes on, as a bit mask.
    pub(super) fn layer_target_slots(&self, layer_index: usize) -> u16 {
        let Some(link) = self.layers.get(layer_index).map(|layer| &layer.link) else {
            return 0;
        };
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
}
