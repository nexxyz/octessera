use super::modulation::apply_sampler_assignments_for_instruments;
use super::{
    note_unit_to_pulses, trigger_probability_allows, NativeRunner, RuntimeTransportState,
    DEFAULT_ALGORITHM_STEP_PULSES, GRID_HEIGHT,
};
use platform_core::{CellTriggerIntent, DeviceInput, MusicalEvent};

impl NativeRunner {
    pub(super) fn active_engine_input_result(
        &mut self,
        input: DeviceInput,
    ) -> Result<platform_core::NativeInputResult, String> {
        if self.transport != RuntimeTransportState::Playing && !self.input_events_while_paused {
            let model = self.engine.on_input(input, self.bpm as f32)?;
            return Ok(platform_core::NativeInputResult {
                events: Vec::new(),
                emitted_events: Vec::new(),
                mapped_intents: Vec::new(),
                model,
            });
        }
        let part_index = self.active_part_index;
        let sense = self.sense_parts.get(part_index).cloned();
        let probability_map = self
            .trigger_probability_maps
            .get(part_index)
            .cloned()
            .unwrap_or_default();
        let mut rng = self.trigger_probability_rng;
        let result =
            self.engine
                .on_input_with_events_filtered(input, self.bpm as f32, |intent| {
                    trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent)
                })?;
        self.trigger_probability_rng = rng;
        Ok(result)
    }

    pub(super) fn active_engine_tick_result(
        &mut self,
    ) -> Result<platform_core::NativeTickResult, String> {
        let (sense, probability_map) = self.probability_context(self.active_part_index);
        let mut rng = self.trigger_probability_rng;
        let result = self.engine.tick_filtered(self.bpm as f32, |intent| {
            trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent)
        })?;
        self.trigger_probability_rng = rng;
        Ok(result)
    }

    pub(super) fn advance_algorithm(&mut self, pulses: u32) -> Result<Vec<MusicalEvent>, String> {
        if pulses == 0 || self.transport != RuntimeTransportState::Playing {
            return Ok(Vec::new());
        }

        let mut events = Vec::new();
        self.advance_transport_indicators(pulses);
        self.accumulate_part_pulses(pulses);
        self.advance_active_part(&mut events)?;

        let instruments = self.instruments.clone();
        let inactive_configs = (0..self.part_engines.len())
            .map(|index| {
                (
                    self.interpretation_profile_for_part(index),
                    self.mapping_config_for_part(index),
                    self.step_pulses_for_part(index),
                    self.sense_parts.get(index).cloned(),
                    self.trigger_probability_maps
                        .get(index)
                        .cloned()
                        .unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        let mut rng = self.trigger_probability_rng;
        let mut inactive_modulation_updates = Vec::new();
        let mut saw_inactive_events = false;
        for (index, engine) in self.part_engines.iter_mut().enumerate() {
            if index == self.active_part_index {
                continue;
            }
            let Some(engine) = engine.as_mut() else {
                continue;
            };
            let (profile, mapping, step_pulses, sense, probability_map) = &inactive_configs[index];
            while self.part_pulse_accumulators[index] >= *step_pulses {
                self.part_pulse_accumulators[index] -= *step_pulses;
                engine.set_interpretation_profile(profile.clone());
                engine.set_mapping_config(mapping.clone());
                let tick = engine.tick_filtered(self.bpm as f32, |intent| {
                    trigger_probability_allows(sense.as_ref(), probability_map, &mut rng, intent)
                })?;
                inactive_modulation_updates.push((index, tick.mapped_intents.clone()));
                let tick_events = apply_sampler_assignments_for_instruments(
                    tick.events,
                    &tick.mapped_intents,
                    tick.emitted_events.len(),
                    &instruments,
                    sense.as_ref(),
                );
                saw_inactive_events |= !tick_events.is_empty();
                events.extend(tick_events);
            }
        }
        self.record_tick_events_active(saw_inactive_events);
        self.trigger_probability_rng = rng;
        for (index, mapped_intents) in inactive_modulation_updates {
            self.apply_runtime_modulation(&mapped_intents, index);
        }
        Ok(events)
    }

    pub(super) fn apply_sampler_assignments(
        &self,
        events: Vec<MusicalEvent>,
        intents: &[CellTriggerIntent],
        part_index: usize,
        mapped_event_offset: usize,
    ) -> Vec<MusicalEvent> {
        apply_sampler_assignments_for_instruments(
            events,
            intents,
            mapped_event_offset,
            &self.instruments,
            self.sense_parts.get(part_index),
        )
    }

    fn step_pulses_for_part(&self, index: usize) -> u32 {
        let Some(sense) = self.sense_parts.get(index) else {
            return self.algorithm_step_pulses;
        };
        if sense.scan_mode == "scanning" {
            note_unit_to_pulses(&sense.scan_unit)
        } else {
            self.part_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES)
        }
    }

    fn probability_context(
        &self,
        part_index: usize,
    ) -> (Option<super::NativeSensePart>, Vec<String>) {
        (
            self.sense_parts.get(part_index).cloned(),
            self.trigger_probability_maps
                .get(part_index)
                .cloned()
                .unwrap_or_default(),
        )
    }

    fn advance_transport_indicators(&mut self, pulses: u32) {
        if self.event_dot_pulses_remaining > 0 {
            self.event_dot_pulses_remaining -= 1;
        }
        self.event_dot_on = self.event_dot_pulses_remaining > 0;
        if self.transport_flash_pulses_remaining > 0 {
            self.transport_flash_pulses_remaining -= 1;
        }
        let previous_pulse = self.current_ppqn_pulse.saturating_sub(u64::from(pulses));
        let current_pulse = self.current_ppqn_pulse;
        if crossed_ppqn_boundary(previous_pulse, current_pulse, 96) {
            self.transport_flash = "measure";
            self.transport_flash_pulses_remaining = 6;
        } else if crossed_ppqn_boundary(previous_pulse, current_pulse, 24) {
            self.transport_flash = "beat";
            self.transport_flash_pulses_remaining = 6;
        } else if self.transport_flash_pulses_remaining == 0 {
            self.transport_flash = "none";
        }
    }

    fn accumulate_part_pulses(&mut self, pulses: u32) {
        if self.part_pulse_accumulators.len() < GRID_HEIGHT {
            self.part_pulse_accumulators.resize(GRID_HEIGHT, 0);
        }
        for value in &mut self.part_pulse_accumulators {
            *value = value.saturating_add(pulses);
        }
    }

    fn advance_active_part(&mut self, events: &mut Vec<MusicalEvent>) -> Result<(), String> {
        let active_step_pulses = self.step_pulses_for_part(self.active_part_index);
        while self.part_pulse_accumulators[self.active_part_index] >= active_step_pulses {
            self.part_pulse_accumulators[self.active_part_index] -= active_step_pulses;
            let tick = self.active_engine_tick_result()?;
            self.tick = self.tick.saturating_add(1);
            self.apply_runtime_modulation(&tick.mapped_intents, self.active_part_index);
            let tick_events = self.apply_sampler_assignments(
                tick.events,
                &tick.mapped_intents,
                self.active_part_index,
                tick.emitted_events.len(),
            );
            self.record_tick_events_active(!tick_events.is_empty());
            events.extend(tick_events);
        }
        Ok(())
    }

    fn record_tick_events_active(&mut self, has_events: bool) {
        if has_events {
            self.event_dot_on = true;
            self.event_dot_pulses_remaining = 1;
        }
    }
}

fn crossed_ppqn_boundary(previous: u64, current: u64, boundary: u64) -> bool {
    boundary > 0 && current >= boundary && previous / boundary != current / boundary
}
