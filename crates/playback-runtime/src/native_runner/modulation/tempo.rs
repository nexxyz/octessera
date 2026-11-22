//! BPM and Swing as modulation targets. A held source (CC, XY, layer axis) sets the tempo the way
//! a menu edit does; an LFO rides on top of it without touching the saved tempo, and only the
//! clock, the behaviors and swing timing hear it.

use super::super::{NativeRunner, Value};

pub(in crate::native_runner) const TEMPO_KEYS: [&str; 3] =
    ["transport.bpm", "transport.swingA", "transport.swingB"];

/// What the LFOs currently add to the set tempo and swing; `None` where no LFO rides.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::native_runner) struct TempoLfo {
    bpm: Option<f64>,
    swing_a: Option<u8>,
    swing_b: Option<u8>,
}

pub(in crate::native_runner) fn is_tempo_key(key: &str) -> bool {
    TEMPO_KEYS.contains(&key)
}

impl NativeRunner {
    /// Sets the transport tempo and retimes what follows it; false when it did not change.
    pub(in crate::native_runner) fn set_transport_bpm(&mut self, bpm: f64) -> bool {
        let bpm = crate::delay_timing::clamp_visible_bpm(bpm);
        if (self.transport.bpm - bpm).abs() <= f64::EPSILON {
            return false;
        }
        self.transport.bpm = bpm;
        self.retime_note_mode_bus_delays();
        self.retime_held_play_fx();
        true
    }

    /// Sets Swing A or B; the clock takes it at the next beat. False when it did not change.
    pub(in crate::native_runner) fn set_transport_swing(&mut self, key: &str, value: i32) -> bool {
        let value = value.clamp(1, 7) as u8;
        let swing = if key == "transport.swingA" {
            &mut self.transport.swing_a
        } else {
            &mut self.transport.swing_b
        };
        let changed = *swing != value;
        *swing = value;
        changed
    }

    /// A held source sets the tempo or swing and its row, so the encoder steps from there.
    pub(super) fn apply_tempo_binding(&mut self, key: &str, value: Value) -> bool {
        let Some(value) = value.as_f64() else {
            return false;
        };
        let changed = if key == "transport.bpm" {
            self.set_transport_bpm(value.round())
        } else {
            self.set_transport_swing(key, value.round() as i32)
        };
        if changed {
            if let Some(value) = self.tempo_base_value(key) {
                self.menu
                    .set_number_value_for_key(key, value.round() as i32);
            }
        }
        changed
    }

    /// The set tempo or swing a modulation composes around.
    pub(in crate::native_runner) fn tempo_base_value(&self, key: &str) -> Option<f64> {
        match key {
            "transport.bpm" => Some(self.transport.bpm),
            "transport.swingA" => Some(f64::from(self.transport.swing_a)),
            "transport.swingB" => Some(f64::from(self.transport.swing_b)),
            _ => None,
        }
    }

    /// Lets the LFOs' composed value for a tempo key play, or drops it when `value` is None.
    pub(super) fn set_tempo_lfo(&mut self, key: &str, value: Option<f64>) {
        let lfo = &mut self.transport.tempo_lfo;
        let swing = value.map(|value| value.round().clamp(1.0, 7.0) as u8);
        match key {
            "transport.bpm" => lfo.bpm = value.map(crate::delay_timing::clamp_visible_bpm),
            "transport.swingA" => lfo.swing_a = swing,
            "transport.swingB" => lfo.swing_b = swing,
            _ => {}
        }
    }

    pub(in crate::native_runner) fn clear_tempo_lfo(&mut self) {
        self.transport.tempo_lfo = TempoLfo::default();
    }

    /// The tempo the clock runs at: the set tempo, or what an LFO makes of it. Under external
    /// sync the incoming clock sets the pace, so an LFO on BPM does nothing.
    pub(in crate::native_runner) fn clock_bpm(&self) -> f64 {
        match self.transport.tempo_lfo.bpm {
            Some(bpm) if self.transport.sync_source != super::super::SyncSource::External => bpm,
            _ => self.transport.bpm,
        }
    }

    /// The swing the next beat takes: the set swing, or what an LFO makes of it.
    pub(in crate::native_runner) fn clock_swing(&self) -> (u8, u8) {
        let lfo = self.transport.tempo_lfo;
        (
            lfo.swing_a.unwrap_or(self.transport.swing_a),
            lfo.swing_b.unwrap_or(self.transport.swing_b),
        )
    }
}
