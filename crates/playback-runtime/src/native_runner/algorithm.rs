use super::modulation::RoutedMusicalEvents;
use super::trigger_probability::trigger_probability_allows;
use super::{
    NativeRunner, RuntimeTransportState, ScanMode, SyncSource, TransportFlash, GRID_HEIGHT,
};
use crate::timing_units::{BAR_PULSES, BEAT_PULSES};
use platform_core::DeviceInput;
use swing::Swing;

pub(super) use super::link_routing::LinkRoutingInput;

mod inactive_layer_steps;
pub(super) mod swing;

impl NativeRunner {
    pub(super) fn active_engine_input_result(
        &mut self,
        input: DeviceInput,
    ) -> Result<platform_core::NativeInputResult, String> {
        self.layer_engine_input_result(self.active_layer_index, input)
    }

    /// Sends `input` to the engine of `layer_index`, active or not, under that layer's trigger
    /// probability.
    pub(super) fn layer_engine_input_result(
        &mut self,
        layer_index: usize,
        input: DeviceInput,
    ) -> Result<platform_core::NativeInputResult, String> {
        let paused = self.transport.transport != RuntimeTransportState::Playing
            && !self.input_events_while_paused;
        let bpm = self.transport.bpm as f32;
        let mut rng = self.probability_rng(layer_index);
        let layer = self
            .layers
            .get_mut(layer_index)
            .ok_or_else(|| format!("layer {} has no engine", layer_index + 1))?;
        if paused {
            let model = layer.engine.on_input(input, bpm)?;
            return Ok(platform_core::NativeInputResult {
                events: Vec::new(),
                emitted_events: Vec::new(),
                mapped_intents: Vec::new(),
                event_intents: Vec::new(),
                model,
            });
        }
        let (link, probability_map) = (&layer.link, &layer.trigger_probability_map);
        let result = layer
            .engine
            .on_input_with_events_filtered(input, bpm, |intent| {
                trigger_probability_allows(Some(link), Some(probability_map), &mut rng, intent)
            })?;
        self.store_probability_rng(layer_index, rng);
        Ok(result)
    }

    pub(super) fn active_engine_tick_result(
        &mut self,
    ) -> Result<platform_core::NativeTickResult, String> {
        let layer_index = self.active_layer_index;
        let mut rng = self.probability_rng(layer_index);
        let layer = &mut self.layers[layer_index];
        let (link, probability_map) = (&layer.link, &layer.trigger_probability_map);
        let result = layer
            .engine
            .tick_filtered(self.transport.bpm as f32, |intent| {
                trigger_probability_allows(Some(link), Some(probability_map), &mut rng, intent)
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

        let inactive_modulation_updates = self.advance_inactive_layers(&mut events)?;
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
        for index in 0..self.layers.len() {
            if self.layer_behavior_id(index) == "sequencer"
                && self
                    .layers
                    .get(index)
                    .map(|layer| &layer.link)
                    .is_some_and(|layer| layer.scan_mode == ScanMode::Scanning)
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
        if let Some(layer) = self.layers.get(index).map(|layer| &layer.link) {
            if layer.scan_mode == ScanMode::Scanning {
                return crate::timing_units::note_unit_to_pulses(&layer.scan_unit);
            }
        }
        self.layer_step_rate_pulses(index)
    }

    /// The layer's Build Step Rate, whether or not it scans.
    pub(super) fn layer_step_rate_pulses(&self, index: usize) -> u32 {
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
        if crossed_ppqn_boundary(previous_pulse, current_pulse, BAR_PULSES) {
            self.display
                .transients
                .trigger_transport_flash(TransportFlash::Measure, now);
        } else if crossed_ppqn_boundary(previous_pulse, current_pulse, BEAT_PULSES) {
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

    /// A swing change waits for the next beat: within a beat every ratio emits the same steps, so
    /// switching only where nothing of the coming beat has been emitted keeps each step once.
    fn consume_swung_pulses(&mut self) -> u32 {
        let current = self.transport.current_ppqn_pulse;
        if current / BEAT_PULSES > self.transport.swung_ppqn_pulse / BEAT_PULSES {
            self.transport.applied_swing = self.swing();
        }
        let swing = self.transport.applied_swing;
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
        let swing = self.transport.applied_swing;
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
            let transpose_offset = self.play_transpose_offset_for_layer(self.active_layer_index);
            let tick_events = self.route_events_with_link_timing(
                self.active_layer_index,
                LinkRoutingInput {
                    events: tick.events,
                    event_intents: &tick.event_intents,
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
