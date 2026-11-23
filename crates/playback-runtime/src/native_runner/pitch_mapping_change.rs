//! A Link change that moves which pitch a tile plays (Root, Set, 1st Note, Low/High Note, Pitch
//! Steps, Range, a section restart, or a modulation of one) releases every key the instruments of
//! the layer's note-on rows hold, at the layer's next step. A change that leaves every pitch where
//! it was, such as an LFO tick that lands on the same Root, releases nothing. The comparison reads
//! exactly what the layer engine maps pitches from: its mapping's pitch fields and the degree
//! layout of its interpretation profile.

use super::{NativeRunner, TriggerAction, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{DegreeLayout, MappingConfig, RangeMode};

/// What a layer's pitches followed when it last played.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PitchMapping {
    base_midi_note: i32,
    starting_midi_note: i32,
    max_midi_note: i32,
    range_mode: RangeMode,
    row_step_degrees: i32,
    column_step_degrees: i32,
    scale: Vec<i32>,
    layout: DegreeLayout,
}

impl PitchMapping {
    fn of(mapping: &MappingConfig, layout: DegreeLayout) -> Self {
        Self {
            base_midi_note: mapping.base_midi_note,
            starting_midi_note: mapping.starting_midi_note,
            max_midi_note: mapping.max_midi_note,
            range_mode: mapping.range_mode,
            row_step_degrees: mapping.row_step_degrees,
            column_step_degrees: mapping.column_step_degrees,
            scale: mapping.scale.clone(),
            layout,
        }
    }

    fn matches(&self, mapping: &MappingConfig, layout: DegreeLayout) -> bool {
        (
            self.base_midi_note,
            self.starting_midi_note,
            self.max_midi_note,
            self.range_mode,
            self.row_step_degrees,
            self.column_step_degrees,
            self.layout,
        ) == (
            mapping.base_midi_note,
            mapping.starting_midi_note,
            mapping.max_midi_note,
            mapping.range_mode,
            mapping.row_step_degrees,
            mapping.column_step_degrees,
            layout,
        ) && self.scale == mapping.scale
    }
}

impl NativeRunner {
    /// The slots whose keys `layer_index` releases because its pitch mapping moved since it last
    /// played; None when it did not, or when no row plays a note.
    pub(super) fn changed_pitch_mapping_targets(&mut self, layer_index: usize) -> Option<u16> {
        let layer = self.layers.get_mut(layer_index)?;
        let mapping = layer.engine.mapping_config();
        let layout = DegreeLayout::of(
            GRID_WIDTH,
            GRID_HEIGHT,
            layer.engine.interpretation_profile(),
        );
        let moved = match &layer.pitch_mapping {
            Some(stored) if stored.matches(mapping, layout) => return None,
            Some(_) => true,
            None => false,
        };
        layer.pitch_mapping = Some(PitchMapping::of(mapping, layout));
        if moved {
            layer.mono_lanes.forget();
        }
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
