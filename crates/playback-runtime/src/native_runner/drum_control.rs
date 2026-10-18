use super::NativeRunner;
use crate::protocol::CellHit;
use platform_core::{CellTriggerIntent, CellTriggerKind};

impl NativeRunner {
    pub(super) fn enter_drum_assign(&mut self, slot: usize, voice: usize) {
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
            || voice >= 8
        {
            return;
        }
        self.play_fx_assign = None;
        self.sample_assign = None;
        self.trigger_probability_assign = None;
        self.drum_assign = Some((slot, voice as u8));
        self.show_toast(format!("Assign V{}: grid", voice + 1));
    }

    pub(super) fn fast_play_drum_slot_key(&mut self, key: &str) -> bool {
        let Some(slot) = self
            .menu
            .value_for_key(key)
            .and_then(|value| {
                value
                    .strip_prefix('I')
                    .and_then(|value| value.split_once(':'))
                    .and_then(|(number, _)| number.parse::<usize>().ok())
            })
            .and_then(|number| number.checked_sub(1))
        else {
            return false;
        };
        if self
            .instruments
            .get(slot)
            .is_none_or(|instrument| instrument.kind != "drum")
        {
            return false;
        }
        self.play_drum_selected_slot = Some(slot);
        true
    }

    pub(super) fn play_drum_cell(&mut self, x: usize, y: usize) {
        if self.display.ui.shift_held
            || self.display.ui.fn_held
            || self.display.ui.combined_modifier_held
        {
            return;
        }
        let slot = self.play_drum_selected_slot.or_else(|| {
            self.instruments
                .iter()
                .position(|instrument| instrument.kind == "drum")
        });
        let Some(slot) = slot else {
            return;
        };
        let intent = CellTriggerIntent {
            x,
            y,
            degree: 0,
            kind: CellTriggerKind::Activate,
        };
        let Some(hit) = super::modulation_sampler::drum_hit_for_intent(
            &self.instruments,
            slot as u8,
            &intent,
            100,
            0,
            None,
        ) else {
            return;
        };
        self.pending.cell_hits.push(hit);
        let now = self.display.transients.now();
        self.display.transients.trigger_event_dot(now);
    }

    pub(super) fn preview_drum_voice(&mut self, slot: usize, voice: usize) {
        if voice >= 8
            || self
                .instruments
                .get(slot)
                .is_none_or(|instrument| instrument.kind != "drum")
        {
            return;
        }
        self.pending.cell_hits.push(CellHit {
            instrument_slot: slot as u8,
            voice: voice as u8,
            cell: 0,
            pitch_cents: 0,
            velocity: 100,
        });
    }
}
