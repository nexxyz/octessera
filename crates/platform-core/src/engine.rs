use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::behaviors::{NativeBehavior, NativeBehaviorState};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use crate::interpretation::{
    interpret_grid, CellTriggerIntent, GridSnapshot, InterpretationProfile,
};
use crate::mapping::{map_intents_to_musical_events, MappingConfig};
use crate::transforms::{apply_global_sound, GlobalSoundConfig, NoteBehavior};
use crate::MusicalEvent;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct NativeLayerEngineConfig {
    pub behavior: NativeBehavior,
    pub behavior_config: Value,
    pub interpretation_profile: InterpretationProfile,
    pub mapping_config: MappingConfig,
    pub global_sound: GlobalSoundConfig,
    pub note_behaviors: Vec<NoteBehavior>,
    pub layer_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeTickResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<CellTriggerIntent>,
    pub event_intents: Vec<Option<CellTriggerIntent>>,
    pub model: BehaviorRenderModel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeInputResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<CellTriggerIntent>,
    pub event_intents: Vec<Option<CellTriggerIntent>>,
    pub model: BehaviorRenderModel,
}

pub struct NativeLayerEngine {
    behavior: NativeBehavior,
    state: NativeBehaviorState,
    interpretation_profile: InterpretationProfile,
    mapping_config: MappingConfig,
    global_sound: GlobalSoundConfig,
    note_behaviors: Vec<NoteBehavior>,
    layer_index: usize,
    tick: usize,
    held_notes: Vec<String>,
}

impl NativeLayerEngine {
    pub fn new(config: NativeLayerEngineConfig) -> Result<Self, String> {
        let state = config.behavior.init(config.behavior_config.clone())?;
        Self::from_state(config, state)
    }

    pub fn from_serialized_state(
        config: NativeLayerEngineConfig,
        state: Value,
    ) -> Result<Self, String> {
        let state = config.behavior.deserialize(state)?;
        Self::from_state(config, state)
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
            layer_index: config.layer_index,
            tick: 0,
            held_notes: Vec::new(),
        })
    }

    pub fn serialized_state(&self) -> Result<Value, String> {
        self.behavior.serialize(&self.state)
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
        mut filter_intent: impl FnMut(&CellTriggerIntent) -> bool,
    ) -> Result<NativeInputResult, String> {
        let before = self.behavior.render_model(&self.state)?;
        let mut context = BehaviorContext::new(bpm);
        self.state = self
            .behavior
            .on_input(self.state.clone(), input, &mut context)?;
        let after = self.behavior.render_model(&self.state)?;
        let mapped = if self.behavior.interpret_input_transitions() {
            let mut profile = self.interpretation_profile.clone();
            profile.state.enabled = false;
            let intents = interpret_grid(
                &to_snapshot(&before),
                &to_snapshot(&after),
                self.tick,
                &profile,
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
        let mapped_event_len = mapped
            .as_ref()
            .map(|mapped| mapped.events.len())
            .unwrap_or(0);
        let mut events = Vec::with_capacity(context.emitted_events.len() + mapped_event_len);
        events.extend(context.emitted_events.iter().cloned());
        let mut event_intents = vec![None; context.emitted_events.len()];
        if let Some(mapped) = &mapped {
            events.extend(mapped.events.iter().cloned());
            event_intents.extend(mapped.event_intents.iter().cloned().map(Some));
        }
        let events = apply_global_sound(&events, &self.global_sound);
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch before note behavior".into());
        }
        let note_behavior = apply_note_behavior_with_event_intents(
            &events,
            event_intents,
            self.note_behaviors.as_slice(),
            self.layer_index,
            &self.held_notes,
        );
        let NoteBehaviorWithIntentsResult {
            events,
            event_intents,
            held_notes,
        } = note_behavior;
        self.held_notes = held_notes;
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch after note behavior".into());
        }
        let mapped_intents = mapped.map(|mapped| mapped.intents).unwrap_or_default();
        Ok(NativeInputResult {
            events,
            emitted_events: context.emitted_events,
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
        mut filter_intent: impl FnMut(&CellTriggerIntent) -> bool,
    ) -> Result<NativeTickResult, String> {
        let before = self.behavior.render_model(&self.state)?;
        let mut context = BehaviorContext::new(bpm);
        self.state = self.behavior.on_tick(self.state.clone(), &mut context)?;
        let after = self.behavior.render_model(&self.state)?;

        let intents = interpret_grid(
            &to_snapshot(&before),
            &to_snapshot(&after),
            self.tick,
            &self.interpretation_profile,
        );
        let intents = intents
            .into_iter()
            .filter(|intent| filter_intent(intent))
            .collect::<Vec<_>>();
        self.tick = self.tick.saturating_add(1);
        let mapped = map_intents_to_musical_events(&intents, &self.mapping_config);
        let mut events = Vec::with_capacity(context.emitted_events.len() + mapped.events.len());
        events.extend(context.emitted_events.iter().cloned());
        let mut event_intents = vec![None; context.emitted_events.len()];
        event_intents.extend(mapped.event_intents.iter().cloned().map(Some));
        events.extend(mapped.events);
        let events = apply_global_sound(&events, &self.global_sound);
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch before note behavior".into());
        }
        let note_behavior = apply_note_behavior_with_event_intents(
            &events,
            event_intents,
            &self.note_behaviors,
            self.layer_index,
            &self.held_notes,
        );
        let NoteBehaviorWithIntentsResult {
            events,
            event_intents,
            held_notes,
        } = note_behavior;
        self.held_notes = held_notes;
        if events.len() != event_intents.len() {
            return Err("event intent metadata length mismatch after note behavior".into());
        }

        Ok(NativeTickResult {
            emitted_events: context.emitted_events,
            events,
            mapped_intents: mapped.intents,
            event_intents,
            model: after,
        })
    }

    pub fn model(&self) -> Result<BehaviorRenderModel, String> {
        self.behavior.render_model(&self.state)
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
    }

    pub fn state(&self) -> &NativeBehaviorState {
        &self.state
    }
}

fn to_snapshot(model: &BehaviorRenderModel) -> GridSnapshot {
    GridSnapshot {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        cells: model.cells.clone(),
        trigger_types: model.trigger_types.clone(),
    }
}

struct NoteBehaviorWithIntentsResult {
    events: Vec<MusicalEvent>,
    event_intents: Vec<Option<CellTriggerIntent>>,
    held_notes: Vec<String>,
}

fn apply_note_behavior_with_event_intents(
    events: &[MusicalEvent],
    event_intents: Vec<Option<CellTriggerIntent>>,
    behaviors: &[NoteBehavior],
    layer_idx: usize,
    initial_held: &[String],
) -> NoteBehaviorWithIntentsResult {
    let mut held = initial_held.iter().cloned().collect::<HashSet<_>>();
    held.reserve(events.len());
    let mut out = Vec::with_capacity(events.len());
    let mut out_intents = Vec::with_capacity(event_intents.len());
    let mut intents = event_intents.into_iter();
    for event in events {
        let event_intent = intents.next().unwrap_or(None);
        match event {
            MusicalEvent::NoteOn {
                channel,
                note,
                velocity,
                duration_ms,
            } => {
                let key = format!("{layer_idx}:{channel}:{note}");
                let behavior = behaviors
                    .get(*channel as usize)
                    .copied()
                    .unwrap_or(NoteBehavior::Oneshot);
                if behavior == NoteBehavior::Hold && held.contains(&key) {
                    continue;
                }
                if behavior == NoteBehavior::Hold {
                    held.insert(key);
                    out.push(MusicalEvent::NoteOn {
                        channel: *channel,
                        note: *note,
                        velocity: *velocity,
                        duration_ms: None,
                    });
                } else {
                    out.push(MusicalEvent::NoteOn {
                        channel: *channel,
                        note: *note,
                        velocity: *velocity,
                        duration_ms: *duration_ms,
                    });
                }
                out_intents.push(event_intent);
            }
            MusicalEvent::NoteOff { channel, note } => {
                let key = format!("{layer_idx}:{channel}:{note}");
                let _ = held.remove(&key);
                out.push(event.clone());
                out_intents.push(event_intent);
            }
            _ => {
                out.push(event.clone());
                out_intents.push(event_intent);
            }
        }
    }
    let mut held_notes = held.into_iter().collect::<Vec<_>>();
    held_notes.sort();
    NoteBehaviorWithIntentsResult {
        events: out,
        event_intents: out_intents,
        held_notes,
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
