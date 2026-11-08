use super::{
    push_tick_events, trigger_probability_allows, LinkRoutingInput, NativeRunner,
    RoutedMusicalEvents, ScanMode,
};
use platform_core::{NativeBehaviorState, NativeLayerEngine, TileTriggerIntent};

type ModulationUpdates = Vec<(usize, Vec<TileTriggerIntent>)>;

impl NativeRunner {
    /// Steps every due layer except the active one and returns each step's intents for runtime
    /// modulation.
    pub(super) fn advance_inactive_layers(
        &mut self,
        events: &mut RoutedMusicalEvents,
    ) -> Result<ModulationUpdates, String> {
        let mut transpose_offsets = None;
        let mut modulation_updates = Vec::new();
        for index in 0..self.layer_engines.len() {
            let step_pulses = self.step_pulses_for_layer(index);
            if index == self.active_layer_index
                || self.transport.layer_pulse_accumulators[index] < step_pulses
            {
                continue;
            }
            let Some(engine) = self.layer_engines[index].as_ref() else {
                let accumulator = &mut self.transport.layer_pulse_accumulators[index];
                while *accumulator >= step_pulses {
                    *accumulator -= step_pulses;
                }
                continue;
            };
            if self.inactive_layer_is_idle(index, engine) {
                self.advance_idle_layer(index, step_pulses, events, &mut modulation_updates);
                continue;
            }
            let transpose_offset = transpose_offsets
                .get_or_insert_with(|| self.play_transpose_offsets_for_routing())
                .get(index)
                .copied()
                .unwrap_or(0);
            self.advance_inactive_layer(
                index,
                step_pulses,
                transpose_offset,
                events,
                &mut modulation_updates,
            )?;
        }
        Ok(modulation_updates)
    }

    /// A `none` layer that does not scan renders an empty grid that never changes, so its ticks
    /// yield no intents, draw no probability and route nothing.
    fn inactive_layer_is_idle(&self, index: usize, engine: &NativeLayerEngine) -> bool {
        matches!(engine.state(), NativeBehaviorState::None(_))
            && self
                .link_layers
                .get(index)
                .is_some_and(|layer| layer.scan_mode != ScanMode::Scanning)
    }

    fn advance_idle_layer(
        &mut self,
        index: usize,
        step_pulses: u32,
        events: &mut RoutedMusicalEvents,
        modulation_updates: &mut ModulationUpdates,
    ) {
        while self.transport.layer_pulse_accumulators[index] >= step_pulses {
            self.transport.layer_pulse_accumulators[index] -= step_pulses;
            let offset = self.swing_offset_nanos(self.transport.layer_pulse_accumulators[index]);
            if let Some(engine) = self.layer_engines[index].as_mut() {
                engine.advance_interpretation_tick();
            }
            self.count_step_and_sound_due_link_events(index, offset, events);
            modulation_updates.push((index, Vec::new()));
        }
    }

    fn advance_inactive_layer(
        &mut self,
        index: usize,
        step_pulses: u32,
        transpose_offset: i8,
        events: &mut RoutedMusicalEvents,
        modulation_updates: &mut ModulationUpdates,
    ) -> Result<(), String> {
        let profile = self.interpretation_profile_for_layer(index);
        let mapping = self.mapping_config_for_layer(index);
        if let Some(engine) = self.layer_engines[index].as_mut() {
            engine.set_global_sound(self.global_sound.clone());
            engine.set_interpretation_profile(profile);
            engine.set_mapping_config(mapping);
        }
        let bpm = self.transport.bpm as f32;
        while self.transport.layer_pulse_accumulators[index] >= step_pulses {
            self.transport.layer_pulse_accumulators[index] -= step_pulses;
            let offset = self.swing_offset_nanos(self.transport.layer_pulse_accumulators[index]);
            let mut rng = self.probability_rng(index);
            let tick = {
                let Some(engine) = self.layer_engines[index].as_mut() else {
                    continue;
                };
                let sense = self.link_layers.get(index);
                let probability_map = self.trigger_probability_maps.get(index);
                engine.tick_filtered(bpm, |intent| {
                    trigger_probability_allows(sense, probability_map, &mut rng, intent)
                })?
            };
            self.store_probability_rng(index, rng);
            self.count_step_and_sound_due_link_events(index, offset, events);
            modulation_updates.push((index, tick.mapped_intents));
            let tick_events = self.route_events_with_link_timing(
                index,
                LinkRoutingInput {
                    events: tick.events,
                    event_intents: &tick.event_intents,
                    transpose_offset,
                },
            )?;
            self.track_emitted_route_notes(index, &tick_events);
            push_tick_events(
                events,
                &mut self.transport.swung_events,
                offset,
                tick_events,
            );
        }
        Ok(())
    }

    fn count_step_and_sound_due_link_events(
        &mut self,
        index: usize,
        offset: u64,
        events: &mut RoutedMusicalEvents,
    ) {
        if let Some(layer_tick) = self.transport.layer_ticks.get_mut(index) {
            *layer_tick = layer_tick.saturating_add(1);
        }
        let due_events = self.take_due_link_events(index);
        self.track_emitted_route_notes(index, &due_events);
        push_tick_events(events, &mut self.transport.swung_events, offset, due_events);
    }
}
