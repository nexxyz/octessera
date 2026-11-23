//! A scan in `mono` lanes plays one note per lane (a row on a column scan, a column on a row
//! scan) and instrument: a note-on from the lane first ends the lane's last note, and a note-off
//! ends that last note rather than the pitch of the tile it fires on. The lane remembers the
//! pitch and the tile that started its last note, so the release names that note's own pitch and
//! sample pad; it is the lane's only memory, and every drain forgets it.

use super::{NativeRunner, ScanAxis, ScanLanes, ScanMode, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{MusicalEvent, TileTriggerIntent, INSTRUMENT_COUNT};

const LANES: usize = if GRID_WIDTH > GRID_HEIGHT {
    GRID_WIDTH
} else {
    GRID_HEIGHT
};

/// The last note each lane of a mono scan played, per instrument slot, with the tile that started
/// it.
#[derive(Clone, Debug)]
pub(super) struct MonoLanes {
    last: [[Option<LaneNote>; INSTRUMENT_COUNT]; LANES],
}

#[derive(Clone, Copy, Debug)]
struct LaneNote {
    pitch: u8,
    x: usize,
    y: usize,
}

impl Default for MonoLanes {
    fn default() -> Self {
        Self {
            last: [[None; INSTRUMENT_COUNT]; LANES],
        }
    }
}

impl MonoLanes {
    pub(super) fn forget(&mut self) {
        *self = Self::default();
    }
}

impl NativeRunner {
    /// The step's events as a mono scan plays them, each with its intent; None when the layer
    /// plays poly lanes or does not scan.
    pub(super) fn mono_lane_events(
        &mut self,
        layer_index: usize,
        events: &[MusicalEvent],
        intents: &[Option<TileTriggerIntent>],
    ) -> Option<(Vec<MusicalEvent>, Vec<Option<TileTriggerIntent>>)> {
        let layer = self.layers.get_mut(layer_index)?;
        let link = &layer.link;
        if link.scan_mode != ScanMode::Scanning || link.scan_lanes != ScanLanes::Mono {
            return None;
        }
        let columns = link.scan_axis == ScanAxis::Columns;
        let mut out_events = Vec::with_capacity(events.len() + 4);
        let mut out_intents = Vec::with_capacity(events.len() + 4);
        for (event, intent) in events.iter().zip(intents) {
            let lane_slot = intent.as_ref().and_then(|intent| {
                let lane = if columns { intent.y } else { intent.x };
                let slot = match *event {
                    MusicalEvent::NoteOn { channel, .. }
                    | MusicalEvent::NoteOff { channel, .. } => usize::from(channel),
                    MusicalEvent::Cc { .. } => return None,
                };
                (lane < LANES && slot < INSTRUMENT_COUNT).then_some((lane, slot))
            });
            let Some((lane, slot)) = lane_slot else {
                out_events.push(event.clone());
                out_intents.push(intent.clone());
                continue;
            };
            let last = &mut layer.mono_lanes.last[lane][slot];
            let previous = match *event {
                MusicalEvent::NoteOn { note, .. } => last.replace(LaneNote {
                    pitch: note,
                    x: intent.as_ref().map_or(0, |intent| intent.x),
                    y: intent.as_ref().map_or(0, |intent| intent.y),
                }),
                MusicalEvent::NoteOff { .. } | MusicalEvent::Cc { .. } => last.take(),
            };
            if let Some(previous) = previous {
                out_events.push(MusicalEvent::NoteOff {
                    channel: slot as u8,
                    note: previous.pitch,
                });
                out_intents.push(intent.clone().map(|mut intent| {
                    (intent.x, intent.y) = (previous.x, previous.y);
                    intent
                }));
            }
            if matches!(event, MusicalEvent::NoteOn { .. }) {
                out_events.push(event.clone());
                out_intents.push(intent.clone());
            }
        }
        Some((out_events, out_intents))
    }
}
