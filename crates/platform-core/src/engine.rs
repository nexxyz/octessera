use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput, StepPhase};
use crate::behavior_random::LayerRandom;
use crate::behaviors::{NativeBehavior, NativeBehaviorState};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use crate::interpretation::{
    interpret_grid_with_marker_authority, GridSnapshot, InterpretationProfile, TileTriggerIntent,
    TileTriggerKind, TriggerMarkerAuthority,
};
use crate::interpretation_scan::compute_degree;
use crate::mapping::{
    activate_tile_for_note, map_intents_to_musical_events, MappingConfig, MappingResult,
    TriggerAction,
};
use crate::transforms::{apply_global_sound, GlobalSoundConfig, NoteBehavior};
use crate::MusicalEvent;
use held_notes::{HeldNotes, NoteBehaviorWithIntentsResult};
use serde_json::Value;

mod held_notes;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLayerEngineConfig {
    pub behavior: NativeBehavior,
    pub behavior_config: Value,
    pub interpretation_profile: InterpretationProfile,
    pub mapping_config: MappingConfig,
    pub global_sound: GlobalSoundConfig,
    pub note_behaviors: Vec<NoteBehavior>,
    pub layer_index: usize,
    pub random_seed: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeTickResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<TileTriggerIntent>,
    pub event_intents: Vec<Option<TileTriggerIntent>>,
    pub model: BehaviorRenderModel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeInputResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<TileTriggerIntent>,
    pub event_intents: Vec<Option<TileTriggerIntent>>,
    pub model: BehaviorRenderModel,
}

pub struct NativeLayerEngine {
    behavior: NativeBehavior,
    state: NativeBehaviorState,
    interpretation_profile: InterpretationProfile,
    mapping_config: MappingConfig,
    global_sound: GlobalSoundConfig,
    note_behaviors: Vec<NoteBehavior>,
    tick: usize,
    held_notes: HeldNotes,
    random: LayerRandom,
    step_phase: StepPhase,
}

impl NativeLayerEngine {
    pub fn new(config: NativeLayerEngineConfig) -> Result<Self, String> {
        let mut random = LayerRandom::new(config.random_seed);
        let behavior = config.behavior;
        let behavior_config = config.behavior_config.clone();
        let state = random.scope(|| behavior.init(behavior_config))?;
        let mut engine = Self::from_state(config, state)?;
        engine.random = random;
        Ok(engine)
    }

    pub fn from_serialized_state(
        config: NativeLayerEngineConfig,
        state: Value,
    ) -> Result<Self, String> {
        let mut random = LayerRandom::new(config.random_seed);
        let state = random.scope(|| config.behavior.deserialize(state))?;
        let mut engine = Self::from_state(config, state)?;
        engine.random = random;
        Ok(engine)
    }

    fn from_state(
        config: NativeLayerEngineConfig,
        state: NativeBehaviorState,
    ) -> Result<Self, String> {
        Ok(Self {
            behavior: config.behavior,
            state,
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            tick: 0,
            held_notes: HeldNotes::default(),
            random: LayerRandom::new(config.random_seed),
            step_phase: StepPhase::Stopped,
        })
    }

    pub fn serialized_state(&self) -> Result<Value, String> {
        self.behavior.serialize(&self.state)
    }

    pub fn capture_persistence_state(&self) -> (NativeBehavior, NativeBehaviorState) {
        (self.behavior, self.state.clone())
    }

    pub fn on_input(
        &mut self,
        input: DeviceInput,
        bpm: f32,
    ) -> Result<BehaviorRenderModel, String> {
        Ok(self.on_input_with_events(input, bpm)?.model)
    }

    pub fn on_input_with_events(
        &mut self,
        input: DeviceInput,
        bpm: f32,
    ) -> Result<NativeInputResult, String> {
        self.on_input_with_events_filtered(input, bpm, |_| true)
    }

    pub fn on_input_with_events_filtered(
        &mut self,
        input: DeviceInput,
        bpm: f32,
        mut filter_intent: impl FnMut(&TileTriggerIntent) -> bool,
    ) -> Result<NativeInputResult, String> {
        let before = self.behavior.render_model(&self.state)?;
        let marker_authority = match &input {
            DeviceInput::GridPress { x, y } if *x < GRID_WIDTH && *y < GRID_HEIGHT => {
                TriggerMarkerAuthority::FreshInput { x: *x, y: *y }
            }
            _ => TriggerMarkerAuthority::LegacyCompatible,
        };
        let mut context = BehaviorContext::new(bpm);
        context.step_phase = self.step_phase;
        let behavior = self.behavior;
        let state = self.state.clone();
        self.state = self
            .random
            .scope(|| behavior.on_input(state, input, &mut context))?;
        let after = self.behavior.render_model(&self.state)?;
        let mapped = if self.behavior.interpret_input_transitions() {
            let mut profile = self.interpretation_profile.clone();
            profile.state.enabled = false;
            let intents = interpret_grid_with_marker_authority(
                &to_snapshot(&before),
                &to_snapshot(&after),
                self.tick,
                &profile,
                marker_authority,
                false,
            );
            let intents = intents
                .into_iter()
                .filter(|intent| filter_intent(intent))
                .collect::<Vec<_>>();
            Some(map_intents_to_musical_events(
                &intents,
                &self.mapping_config,
            ))
        } else {
            None
        };
        let emitted_events = context.emitted_events;
        let finalized = self.finalize_events(&emitted_events, mapped.as_ref())?;
        let NoteBehaviorWithIntentsResult {
            events,
            event_intents,
        } = finalized;
        let mapped_intents = mapped.map(|mapped| mapped.intents).unwrap_or_default();
        Ok(NativeInputResult {
            events,
            emitted_events,
            mapped_intents,
            event_intents,
            model: after,
        })
    }

    pub fn tick(&mut self, bpm: f32) -> Result<NativeTickResult, String> {
        self.tick_filtered(bpm, |_| true)
    }

    pub fn tick_filtered(
        &mut self,
        bpm: f32,
        mut filter_intent: impl FnMut(&TileTriggerIntent) -> bool,
    ) -> Result<NativeTickResult, String> {
        let before = self.behavior.render_model(&self.state)?;
        let mut context = BehaviorContext::new(bpm);
        let behavior = self.behavior;
        let state = self.state.clone();
        self.state = self
            .random
            .scope(|| behavior.on_tick(state, &mut context))?;
        let after = self.behavior.render_model(&self.state)?;

        let stable = self.behavior.reports_stable_tiles()
            && self.mapping_config.stable.action != TriggerAction::None;
        let intents = interpret_grid_with_marker_authority(
            &to_snapshot(&before),
            &to_snapshot(&after),
            self.tick,
            &self.interpretation_profile,
            TriggerMarkerAuthority::FreshTick,
            stable,
        );
        let intents = intents
            .into_iter()
            .filter(|intent| filter_intent(intent))
            .collect::<Vec<_>>();
        self.tick = self.tick.saturating_add(1);
        let mapped = map_intents_to_musical_events(&intents, &self.mapping_config);
        let emitted_events = context.emitted_events;
        let finalized = self.finalize_events(&emitted_events, Some(&mapped))?;
        let NoteBehaviorWithIntentsResult {
            mut events,
            mut event_intents,
        } = finalized;
        if !self.interpretation_profile.event.enabled {
            // Without Off events nothing else ends a note a tile holds once the tile is dark.
            for (tile, off) in self.held_notes.release_dark_tiles(&after.tiles) {
                let (x, y) = (tile % GRID_WIDTH, tile / GRID_WIDTH);
                events.push(off);
                event_intents.push(Some(TileTriggerIntent {
                    x,
                    y,
                    kind: TileTriggerKind::Stable,
                    degree: compute_degree(
                        GRID_WIDTH,
                        GRID_HEIGHT,
                        x,
                        y,
                        &self.interpretation_profile,
                    ),
                }));
            }
        }
        let released = self.held_notes.release_unscanned();
        let (events, event_intents) = if released.is_empty() {
            (events, event_intents)
        } else {
            let mut intents = vec![None; released.len()];
            intents.extend(event_intents);
            (released.into_iter().chain(events).collect(), intents)
        };

        Ok(NativeTickResult {
            emitted_events,
            events,
            mapped_intents: mapped.intents,
            event_intents,
            model: after,
        })
    }

    pub fn model(&self) -> Result<BehaviorRenderModel, String> {
        self.behavior.render_model(&self.state)
    }

    pub fn drain_held_notes(&mut self, max_notes: usize) -> Vec<MusicalEvent> {
        self.held_notes.drain(max_notes)
    }

    pub fn set_mapping_config(&mut self, mapping_config: MappingConfig) {
        self.mapping_config = mapping_config;
    }

    pub fn set_interpretation_profile(&mut self, interpretation_profile: InterpretationProfile) {
        self.interpretation_profile = interpretation_profile;
    }

    pub fn set_global_sound(&mut self, global_sound: GlobalSoundConfig) {
        self.global_sound = global_sound;
    }

    pub fn set_note_behaviors(&mut self, note_behaviors: Vec<NoteBehavior>) {
        self.note_behaviors = note_behaviors;
    }

    pub fn reset_transport_phase(&mut self) {
        self.tick = 0;
        let state = &mut self.state;
        self.random.scope(|| state.reset_transport_phase());
    }

    pub fn set_random_seed(&mut self, seed: Option<u64>) {
        if self.random.seed() != seed {
            self.random = LayerRandom::new(seed);
        }
    }

    pub fn restart_random(&mut self) {
        self.random.restart();
    }

    pub fn set_interpretation_tick(&mut self, tick: usize) {
        self.tick = tick;
    }

    /// Counts one step without ticking the behavior and returns the note-offs of scan-held notes
    /// that the step no longer plays.
    pub fn advance_interpretation_tick(&mut self) -> Vec<MusicalEvent> {
        self.tick = self.tick.saturating_add(1);
        self.held_notes.release_unscanned()
    }

    /// Forgets what this layer holds on `channel` once the runner has cut those voices itself.
    pub fn forget_held_channel(&mut self, channel: u8) {
        self.held_notes.forget_channel(channel);
    }

    pub fn state(&self) -> &NativeBehaviorState {
        &self.state
    }

    pub fn behavior(&self) -> NativeBehavior {
        self.behavior
    }

    pub fn interpretation_profile(&self) -> &InterpretationProfile {
        &self.interpretation_profile
    }

    pub fn mapping_config(&self) -> &MappingConfig {
        &self.mapping_config
    }

    /// True for an overdubbing Looper whose tile presses play instrument `channel`.
    pub fn records_outside_notes_for(&self, channel: u8) -> bool {
        let activate = &self.mapping_config.activate;
        self.state.records_outside_tiles()
            && activate.action == TriggerAction::NoteOn
            && activate.channel.min(15) == channel
    }

    /// The lowest-index tile whose press this layer's mapping plays as `note`.
    pub fn tile_for_note(&self, note: u8) -> Option<usize> {
        activate_tile_for_note(&self.interpretation_profile, &self.mapping_config, note)
    }

    /// Where the transport is inside this layer's step for the inputs that follow.
    pub fn set_step_phase(&mut self, phase: StepPhase) {
        self.step_phase = phase;
    }

    /// Records a press or release of `tile` played elsewhere; false unless overdubbing.
    pub fn record_outside_tile(&mut self, tile: usize, pressed: bool, phase: StepPhase) -> bool {
        self.state.record_outside_tile(tile, pressed, phase)
    }

    fn finalize_events(
        &mut self,
        emitted_events: &[MusicalEvent],
        mapped: Option<&MappingResult>,
    ) -> Result<NoteBehaviorWithIntentsResult, String> {
        let mapped_event_len = mapped.map(|mapped| mapped.events.len()).unwrap_or(0);
        let mut events = Vec::with_capacity(emitted_events.len() + mapped_event_len);
        events.extend(emitted_events.iter().cloned());
        let mut event_intents = vec![None; emitted_events.len()];
        if let Some(mapped) = mapped {
            events.extend(mapped.events.iter().cloned());
            event_intents.extend(mapped.event_intents.iter().cloned().map(Some));
        }
        let events = apply_global_sound(&events, &self.global_sound);
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch before note behavior".into());
        }
        let NoteBehaviorWithIntentsResult {
            events,
            event_intents,
        } = self
            .held_notes
            .apply(&events, event_intents, &self.note_behaviors);
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch after note behavior".into());
        }
        Ok(NoteBehaviorWithIntentsResult {
            events,
            event_intents,
        })
    }
}

fn to_snapshot(model: &BehaviorRenderModel) -> GridSnapshot {
    GridSnapshot {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        tiles: model.tiles.clone(),
        trigger_types: model.trigger_types.clone(),
    }
}

#[cfg(test)]
mod tests;
