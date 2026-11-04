use super::modulation::RoutedMusicalEvents;
use super::trigger_probability::trigger_probability_allows;
use super::{NativeRunner, RuntimeTransportState, SyncSource, TransportFlash, GRID_HEIGHT};
use platform_core::DeviceInput;
use swing::Swing;

pub(super) use super::link_routing::LinkRoutingInput;

pub(super) mod swing;

impl NativeRunner {
    pub(super) fn active_engine_input_result(
        &mut self,
        input: DeviceInput,
    ) -> Result<platform_core::NativeInputResult, String> {
        if self.transport.transport != RuntimeTransportState::Playing
            && !self.input_events_while_paused
        {
            let model = self.engine.on_input(input, self.transport.bpm as f32)?;
            return Ok(platform_core::NativeInputResult {
                events: Vec::new(),
                emitted_events: Vec::new(),
                mapped_intents: Vec::new(),
                event_intents: Vec::new(),
                model,
            });
        }
        let layer_index = self.active_layer_index;
        let sense = self.link_layers.get(layer_index).cloned();
        let probability_map = self
            .trigger_probability_maps
            .get(layer_index)
            .cloned()
            .unwrap_or_default();
        let mut rng = self.probability_rng(layer_index);
        let result = self.engine.on_input_with_events_filtered(
            input,
            self.transport.bpm as f32,
            |intent| trigger_probability_allows(sense.as_ref(), &probability_map, &mut rng, intent),
        )?;
        self.store_probability_rng(layer_index, rng);
        Ok(result)
    }

    pub(super) fn active_engine_tick_result(
        &mut self,
    ) -> Result<platform_core::NativeTickResult, String> {
        let layer_index = self.active_layer_index;
        let mut rng = self.probability_rng(layer_index);
        let sense = self.link_layers.get(layer_index);
        let probability_map = self
            .trigger_probability_maps
            .get(layer_index)
            .map_or(&[][..], Vec::as_slice);
        let result = self
            .engine
            .tick_filtered(self.transport.bpm as f32, |intent| {
                trigger_probability_allows(sense, probability_map, &mut rng, intent)
            })?;
        self.store_probability_rng(layer_index, rng);
        Ok(result)
    }

    pub(super) fn advance_algorithm(&mut self, pulses: u32) -> Result<RoutedMusicalEvents, String> {
        if pulses == 0 || self.transport.transport != RuntimeTransportState::Playing {
            return Ok(RoutedMusicalEvents::default());
        }

        self.advance_transport_indicators(pulses);
        self.advance_global_lfo_audio(pulses)?;
        let swung_pulses = self.consume_swung_pulses();
        self.accumulate_layer_pulses(swung_pulses);
        self.apply_due_rate_resyncs(pulses);
        self.advance_due_layer_ticks()
    }

    pub(super) fn advance_due_layer_ticks(&mut self) -> Result<RoutedMusicalEvents, String> {
        let mut events = RoutedMusicalEvents::default();
        self.advance_active_layer(&mut events)?;

        let mut transpose_offsets = None;
        let mut effective_sound = None;
        let effective_bpm = self.transport.bpm as f32;
        let mut inactive_modulation_updates = Vec::new();
        for index in 0..self.layer_engines.len() {
            let step_pulses = self.step_pulses_for_layer(index);
            if index == self.active_layer_index
                || self.transport.layer_pulse_accumulators[index] < step_pulses
            {
                continue;
            }
            let profile = self.interpretation_profile_for_layer(index);
            let mapping = self.mapping_config_for_layer(index);
            let transpose_offset = transpose_offsets
                .get_or_insert_with(|| self.play_transpose_offsets_for_routing())
                .get(index)
                .copied()
                .unwrap_or(0);
            let effective_sound =
                &*effective_sound.get_or_insert_with(|| self.global_sound.clone());
            while self.transport.layer_pulse_accumulators[index] >= step_pulses {
                self.transport.layer_pulse_accumulators[index] -= step_pulses;
                let offset =
                    self.swing_offset_nanos(self.transport.layer_pulse_accumulators[index]);
                let mut rng = self.probability_rng(index);
                let tick = {
                    let Some(engine) = self.layer_engines[index].as_mut() else {
                        continue;
                    };
                    engine.set_global_sound(effective_sound.clone());
                    engine.set_interpretation_profile(profile.clone());
                    engine.set_mapping_config(mapping.clone());
                    let sense = self.link_layers.get(index);
                    let probability_map = self
                        .trigger_probability_maps
                        .get(index)
                        .map_or(&[][..], Vec::as_slice);
                    engine.tick_filtered(effective_bpm, |intent| {
                        trigger_probability_allows(sense, probability_map, &mut rng, intent)
                    })?
                };
                self.store_probability_rng(index, rng);
                if let Some(layer_tick) = self.transport.layer_ticks.get_mut(index) {
                    *layer_tick = layer_tick.saturating_add(1);
                }
                let due_events = self.take_due_link_events(index);
                self.track_emitted_route_notes(index, &due_events);
                push_tick_events(
                    &mut events,
                    &mut self.transport.swung_events,
                    offset,
                    due_events,
                );
                inactive_modulation_updates.push((index, tick.mapped_intents.clone()));
                let tick_events = self.route_events_with_link_timing(
                    index,
                    LinkRoutingInput {
                        events: tick.events,
                        event_intents: &tick.event_intents,
                        sense: self.link_layers.get(index).cloned(),
                        transpose_offset,
                    },
                )?;
                self.track_emitted_route_notes(index, &tick_events);
                push_tick_events(
                    &mut events,
                    &mut self.transport.swung_events,
                    offset,
                    tick_events,
                );
            }
        }
        for (index, mapped_intents) in inactive_modulation_updates {
            self.apply_runtime_modulation(&mapped_intents, index);
        }
        if !events.is_empty() || !self.transport.swung_events.is_empty() {
            let now = self.display.transients.now();
            self.display.transients.trigger_event_dot(now);
        }
        Ok(events)
    }

    pub(super) fn prime_sequencer_layer_origins(&mut self) {
        for index in 0..self.layer_engines.len() {
            let is_sequencer = if index == self.active_layer_index {
                self.behavior.id() == "sequencer"
            } else {
                self.layer_behavior_ids
                    .get(index)
                    .is_some_and(|behavior_id| behavior_id == "sequencer")
            };
            if is_sequencer
                && self
                    .link_layers
                    .get(index)
                    .is_some_and(|layer| layer.scan_mode == "scanning")
            {
                let step_pulses = self.step_pulses_for_layer(index);
                if let Some(accumulator) = self.transport.layer_pulse_accumulators.get_mut(index) {
                    *accumulator = step_pulses;
                }
            }
        }
        let swing_lead = self.consume_swung_pulses();
        if swing_lead > 0 {
            self.accumulate_layer_pulses(swing_lead);
        }
    }

    pub(super) fn step_pulses_for_layer(&self, index: usize) -> u32 {
        if let Some(layer) = self.link_layers.get(index) {
            if layer.scan_mode == "scanning" {
                return crate::timing_units::note_unit_to_pulses(&layer.scan_unit);
            }
        }
        if index == self.active_layer_index {
            self.transport.algorithm_step_pulses
        } else {
            self.transport
                .layer_algorithm_step_pulses
                .get(index)
                .copied()
                .unwrap_or(super::DEFAULT_ALGORITHM_STEP_RED)
        }
    }

    fn advance_transport_indicators(&mut self, pulses: u32) {
        let now = self.display.transients.now();
        self.display.transients.advance(now);
        let previous_pulse = self
            .transport
            .current_ppqn_pulse
            .saturating_sub(u64::from(pulses));
        let current_pulse = self.transport.current_ppqn_pulse;
        if crossed_ppqn_boundary(previous_pulse, current_pulse, 96) {
            self.display
                .transients
                .trigger_transport_flash(TransportFlash::Measure, now);
        } else if crossed_ppqn_boundary(previous_pulse, current_pulse, 24) {
            self.display
                .transients
                .trigger_transport_flash(TransportFlash::Beat, now);
        }
    }

    fn accumulate_layer_pulses(&mut self, pulses: u32) {
        if self.transport.layer_pulse_accumulators.len() < GRID_HEIGHT {
            self.transport
                .layer_pulse_accumulators
                .resize(GRID_HEIGHT, 0);
        }
        for value in &mut self.transport.layer_pulse_accumulators {
            *value = value.saturating_add(pulses);
        }
    }

    pub(super) fn swing(&self) -> Swing {
        Swing::new(self.transport.swing_a, self.transport.swing_b)
    }

    fn consume_swung_pulses(&mut self) -> u32 {
        let swing = self.swing();
        let current = self.transport.current_ppqn_pulse;
        let target = if swing.is_straight() {
            current
        } else if self.transport.sync_source == SyncSource::External {
            swing.reached_warped_pulse(current)
        } else {
            swing.warped_pulse(current)
        };
        let swung = target.saturating_sub(self.transport.swung_ppqn_pulse);
        self.transport.swung_ppqn_pulse = self.transport.swung_ppqn_pulse.max(target);
        swung.min(u64::from(u32::MAX)) as u32
    }

    /// How long after this pulse step's stamp a layer step sounds whose accumulator now holds
    /// `accumulator` warped pulses.
    fn swing_offset_nanos(&self, accumulator: u32) -> u64 {
        let swing = self.swing();
        if swing.is_straight() {
            return 0;
        }
        let warped = self
            .transport
            .swung_ppqn_pulse
            .saturating_sub(u64::from(accumulator));
        swing.offset_nanos(
            warped,
            self.transport.current_ppqn_pulse,
            self.transport.bpm,
        )
    }

    fn advance_active_layer(&mut self, events: &mut RoutedMusicalEvents) -> Result<(), String> {
        let active_step_pulses = self.step_pulses_for_layer(self.active_layer_index);
        while self.transport.layer_pulse_accumulators[self.active_layer_index] >= active_step_pulses
        {
            self.transport.layer_pulse_accumulators[self.active_layer_index] -= active_step_pulses;
            let offset = self.swing_offset_nanos(
                self.transport.layer_pulse_accumulators[self.active_layer_index],
            );
            let tick = self.active_engine_tick_result()?;
            self.transport.tick = self.transport.tick.saturating_add(1);
            if let Some(layer_tick) = self.transport.layer_ticks.get_mut(self.active_layer_index) {
                *layer_tick = self.transport.tick;
            }
            let due_events = self.take_due_link_events(self.active_layer_index);
            self.track_emitted_route_notes(self.active_layer_index, &due_events);
            push_tick_events(events, &mut self.transport.swung_events, offset, due_events);
            self.apply_runtime_modulation(&tick.mapped_intents, self.active_layer_index);
            let transpose_offset = self
                .play_transpose_offsets_for_routing()
                .get(self.active_layer_index)
                .copied()
                .unwrap_or(0);
            let sense = self.link_layers.get(self.active_layer_index).cloned();
            let tick_events = self.route_events_with_link_timing(
                self.active_layer_index,
                LinkRoutingInput {
                    events: tick.events,
                    event_intents: &tick.event_intents,
                    sense,
                    transpose_offset,
                },
            )?;
            self.track_emitted_route_notes(self.active_layer_index, &tick_events);
            push_tick_events(
                events,
                &mut self.transport.swung_events,
                offset,
                tick_events,
            );
        }
        Ok(())
    }
}

fn crossed_ppqn_boundary(previous: u64, current: u64, boundary: u64) -> bool {
    boundary > 0 && current >= boundary && previous / boundary != current / boundary
}

fn push_tick_events(
    events: &mut RoutedMusicalEvents,
    swung: &mut Vec<(u64, RoutedMusicalEvents)>,
    offset_nanos: u64,
    tick_events: RoutedMusicalEvents,
) {
    if offset_nanos == 0 || tick_events.is_empty() {
        events.extend(tick_events);
        return;
    }
    match swung.last_mut() {
        Some((offset, group)) if *offset == offset_nanos => group.extend(tick_events),
        _ => swung.push((offset_nanos, tick_events)),
    }
}
