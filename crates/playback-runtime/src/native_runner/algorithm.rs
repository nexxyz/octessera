use super::modulation::{apply_sampler_assignments_for_instruments_routed, RoutedMusicalEvents};
use super::{
    note_unit_to_pulses, trigger_probability_allows, NativeRunner, RuntimeTransportState,
    DEFAULT_ALGORITHM_STEP_PULSES, GRID_HEIGHT,
};
use platform_core::DeviceInput;

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
        let layer_index = self.active_layer_index;
        let sense = self.pulses_layers.get(layer_index).cloned();
        let probability_map = self
            .trigger_probability_maps
            .get(layer_index)
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
        let (sense, probability_map) = self.probability_context(self.active_layer_index);
        let mut rng = self.trigger_probability_rng;
        let result = self.engine.tick_filtered(self.bpm as f32, |intent| {
            trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent)
        })?;
        self.trigger_probability_rng = rng;
        Ok(result)
    }

    pub(super) fn advance_algorithm(&mut self, pulses: u32) -> Result<RoutedMusicalEvents, String> {
        if pulses == 0 || self.transport != RuntimeTransportState::Playing {
            return Ok(RoutedMusicalEvents::default());
        }

        let mut events = RoutedMusicalEvents::default();
        self.advance_transport_indicators(pulses);
        let swung_pulses = self.consume_swung_pulses(pulses);
        self.accumulate_layer_pulses(swung_pulses);
        self.advance_active_layer(&mut events)?;

        let instruments = self.instruments.clone();
        let transpose_offsets = self.sparks_transpose_offsets_for_routing();
        let inactive_configs = (0..self.layer_engines.len())
            .map(|index| {
                (
                    self.interpretation_profile_for_layer(index),
                    self.mapping_config_for_layer(index),
                    self.step_pulses_for_layer(index),
                    self.pulses_layers.get(index).cloned(),
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
        for (index, engine) in self.layer_engines.iter_mut().enumerate() {
            if index == self.active_layer_index {
                continue;
            }
            let Some(engine) = engine.as_mut() else {
                continue;
            };
            let (profile, mapping, step_pulses, sense, probability_map) = &inactive_configs[index];
            while self.layer_pulse_accumulators[index] >= *step_pulses {
                self.layer_pulse_accumulators[index] -= *step_pulses;
                engine.set_interpretation_profile(profile.clone());
                engine.set_mapping_config(mapping.clone());
                let tick = engine.tick_filtered(self.bpm as f32, |intent| {
                    trigger_probability_allows(sense.as_ref(), probability_map, &mut rng, intent)
                })?;
                if let Some(layer_tick) = self.layer_ticks.get_mut(index) {
                    *layer_tick = layer_tick.saturating_add(1);
                }
                inactive_modulation_updates.push((index, tick.mapped_intents.clone()));
                let tick_events = apply_sampler_assignments_for_instruments_routed(
                    tick.events,
                    &tick.mapped_intents,
                    tick.emitted_events.len(),
                    &instruments,
                    sense.as_ref(),
                    transpose_offsets.get(index).copied().unwrap_or(0),
                    self.sparks_transpose_active_notes.get_mut(index),
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
        events.dedupe_note_ons_by_highest_velocity();
        Ok(events)
    }

    fn step_pulses_for_layer(&self, index: usize) -> u32 {
        let Some(sense) = self.pulses_layers.get(index) else {
            return self.algorithm_step_pulses;
        };
        if sense.scan_mode == "scanning" {
            note_unit_to_pulses(&sense.scan_unit)
        } else {
            self.layer_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(DEFAULT_ALGORITHM_STEP_PULSES)
        }
    }

    fn probability_context(
        &self,
        layer_index: usize,
    ) -> (Option<super::NativePulsesLayer>, Vec<String>) {
        (
            self.pulses_layers.get(layer_index).cloned(),
            self.trigger_probability_maps
                .get(layer_index)
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

    fn accumulate_layer_pulses(&mut self, pulses: u32) {
        if self.layer_pulse_accumulators.len() < GRID_HEIGHT {
            self.layer_pulse_accumulators.resize(GRID_HEIGHT, 0);
        }
        for value in &mut self.layer_pulse_accumulators {
            *value = value.saturating_add(pulses);
        }
    }

    fn consume_swung_pulses(&mut self, straight_pulses: u32) -> u32 {
        if self.swing_pct == 0 || straight_pulses == 0 {
            self.swung_ppqn_pulse = self.current_ppqn_pulse;
            return straight_pulses;
        }
        let previous = self
            .current_ppqn_pulse
            .saturating_sub(u64::from(straight_pulses));
        let previous_swung = swung_pulse_total(previous, self.swing_pct);
        let current_swung = swung_pulse_total(self.current_ppqn_pulse, self.swing_pct);
        self.swung_ppqn_pulse = current_swung;
        current_swung
            .saturating_sub(previous_swung)
            .min(u64::from(u32::MAX)) as u32
    }

    fn advance_active_layer(&mut self, events: &mut RoutedMusicalEvents) -> Result<(), String> {
        let active_step_pulses = self.step_pulses_for_layer(self.active_layer_index);
        while self.layer_pulse_accumulators[self.active_layer_index] >= active_step_pulses {
            self.layer_pulse_accumulators[self.active_layer_index] -= active_step_pulses;
            let tick = self.active_engine_tick_result()?;
            self.tick = self.tick.saturating_add(1);
            if let Some(layer_tick) = self.layer_ticks.get_mut(self.active_layer_index) {
                *layer_tick = self.tick;
            }
            self.apply_runtime_modulation(&tick.mapped_intents, self.active_layer_index);
            let transpose_offset = self
                .sparks_transpose_offsets_for_routing()
                .get(self.active_layer_index)
                .copied()
                .unwrap_or(0);
            let active_transpose_notes = self
                .sparks_transpose_active_notes
                .get_mut(self.active_layer_index);
            let tick_events = apply_sampler_assignments_for_instruments_routed(
                tick.events,
                &tick.mapped_intents,
                tick.emitted_events.len(),
                &self.instruments,
                self.pulses_layers.get(self.active_layer_index),
                transpose_offset,
                active_transpose_notes,
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

fn swung_pulse_total(pulse: u64, swing_pct: u8) -> u64 {
    let beat = pulse / 24;
    let phase = (pulse % 24) as u32;
    let delay = ((u32::from(swing_pct.min(75)) * 6) + 50) / 100;
    let swung_phase = if delay == 0 || phase < 12 {
        phase
    } else if phase < 12 + delay {
        12
    } else {
        12 + ((phase - 12 - delay) * 12) / (12 - delay)
    };
    beat * 24 + u64::from(swung_phase.min(23))
}
