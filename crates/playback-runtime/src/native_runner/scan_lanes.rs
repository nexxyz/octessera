//! A scan in `mono` lanes plays one note per lane (a row on a column scan, a column on a row
//! scan) and instrument: a note-on from the lane first ends the lane's last note, and a note-off
//! ends that last note rather than the pitch of the cell it fires on. The last note is the
//! lane's only memory; drains and pitch-mapping changes forget it, and so do the scan setting
//! changes that move the lanes.

use super::held_keys::KeyEvent;
use super::modulation::RoutedMusicalEvents;
use super::{NativeRunner, ScanAxis, ScanLanes, ScanMode, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{MusicalEvent, TileTriggerIntent, INSTRUMENT_COUNT};

const LANES: usize = if GRID_WIDTH > GRID_HEIGHT {
    GRID_WIDTH
} else {
    GRID_HEIGHT
};

/// The last note each lane of a mono scan played, per instrument slot.
#[derive(Clone, Debug)]
pub(super) struct MonoLanes {
    last: [[Option<u8>; INSTRUMENT_COUNT]; LANES],
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
            match *event {
                MusicalEvent::NoteOn { channel, note, .. } => {
                    if let Some(previous) = last.replace(note) {
                        out_events.push(MusicalEvent::NoteOff {
                            channel,
                            note: previous,
                        });
                        out_intents.push(intent.clone());
                    }
                    out_events.push(event.clone());
                    out_intents.push(intent.clone());
                }
                MusicalEvent::NoteOff { channel, .. } => {
                    if let Some(previous) = last.take() {
                        out_events.push(MusicalEvent::NoteOff {
                            channel,
                            note: previous,
                        });
                        out_intents.push(intent.clone());
                    }
                }
                MusicalEvent::Cc { .. } => {}
            }
        }
        Some((out_events, out_intents))
    }

    /// What a changed scan setting `field` does, from the menu or a binding alike. A new Scan
    /// Axis, or Scan Mode turned off, drains the layer and starts the line as if it had seen only
    /// dark tiles; Scan Mode turned on starts the line afresh. Sections and Lanes end each mono
    /// lane's last note, since lanes may split, merge or stop playing one note. Scan Dir and Scan
    /// Unit leave every note and the lanes as they are.
    pub(super) fn apply_scan_setting_change(&mut self, layer_index: usize, field: &str) {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        match field {
            "scanMode" if layer.link.scan_mode == ScanMode::Scanning => {
                layer.engine.forget_scanned_line();
                layer.mono_lanes.forget();
            }
            "scanMode" | "scanAxis" => {
                layer.engine.forget_scanned_line();
                self.drain_layer_owned_notes(layer_index);
            }
            "scanSections" | "scanLanes" => self.end_mono_lanes(layer_index),
            _ => {}
        }
    }

    /// Ends the last note of each of the layer's mono lanes and forgets them.
    fn end_mono_lanes(&mut self, layer_index: usize) {
        let Some(layer) = self.layers.get_mut(layer_index) else {
            return;
        };
        let mut ended = RoutedMusicalEvents::default();
        for lane in &layer.mono_lanes.last {
            for (slot, note) in lane.iter().enumerate() {
                if let (Some(pitch), Ok(slot)) = (note, u8::try_from(slot)) {
                    ended.keys.push(KeyEvent::Release {
                        slot,
                        pitch: *pitch,
                    });
                }
            }
        }
        layer.mono_lanes.forget();
        self.sound_keys(&mut ended);
        self.transpose.pending_note_offs.extend(ended);
    }
}

/// The Link fields of the Scanning group.
pub(super) fn is_scan_setting(field: &str) -> bool {
    matches!(
        field,
        "scanMode" | "scanAxis" | "scanUnit" | "scanDirection" | "scanSections" | "scanLanes"
    )
}
