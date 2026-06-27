use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use crate::behaviors::{NativeBehavior, NativeBehaviorState};
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use crate::interpretation::{
    interpret_grid, CellTriggerIntent, GridSnapshot, InterpretationProfile,
};
use crate::mapping::{map_intents_to_musical_events, MappingConfig};
use crate::transforms::{
    apply_global_sound, apply_note_behavior, dedupe_simultaneous_notes, GlobalSoundConfig,
    NoteBehavior, NoteBehaviorResult,
};
use crate::MusicalEvent;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct NativePartEngineConfig {
    pub behavior: NativeBehavior,
    pub behavior_config: Value,
    pub interpretation_profile: InterpretationProfile,
    pub mapping_config: MappingConfig,
    pub global_sound: GlobalSoundConfig,
    pub note_behaviors: Vec<NoteBehavior>,
    pub part_index: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeTickResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<CellTriggerIntent>,
    pub model: BehaviorRenderModel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeInputResult {
    pub events: Vec<MusicalEvent>,
    pub emitted_events: Vec<MusicalEvent>,
    pub mapped_intents: Vec<CellTriggerIntent>,
    pub model: BehaviorRenderModel,
}

pub struct NativePartEngine {
    behavior: NativeBehavior,
    state: NativeBehaviorState,
    interpretation_profile: InterpretationProfile,
    mapping_config: MappingConfig,
    global_sound: GlobalSoundConfig,
    note_behaviors: Vec<NoteBehavior>,
    part_index: usize,
    tick: usize,
    held_notes: Vec<String>,
}

impl NativePartEngine {
    pub fn new(config: NativePartEngineConfig) -> Result<Self, String> {
        let state = config.behavior.init(config.behavior_config.clone())?;
        Self::from_state(config, state)
    }

    pub fn from_serialized_state(
        config: NativePartEngineConfig,
        state: Value,
    ) -> Result<Self, String> {
        let state = config.behavior.deserialize(state)?;
        Self::from_state(config, state)
    }

    fn from_state(
        config: NativePartEngineConfig,
        state: NativeBehaviorState,
    ) -> Result<Self, String> {
        Ok(Self {
            behavior: config.behavior,
            state,
            interpretation_profile: config.interpretation_profile,
            mapping_config: config.mapping_config,
            global_sound: config.global_sound,
            note_behaviors: config.note_behaviors,
            part_index: config.part_index,
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
        if let Some(mapped) = &mapped {
            events.extend(mapped.events.iter().cloned());
        }
        let events = apply_global_sound(&events, &self.global_sound);
        let note_behavior = apply_note_behavior(
            &events,
            self.note_behaviors.as_slice(),
            self.part_index,
            &self.held_notes,
        );
        let NoteBehaviorResult { events, held_notes } = note_behavior;
        self.held_notes = held_notes;
        Ok(NativeInputResult {
            events: dedupe_simultaneous_notes(&events),
            emitted_events: context.emitted_events,
            mapped_intents: mapped.map(|mapped| mapped.intents).unwrap_or_default(),
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
        events.extend(mapped.events);
        let events = apply_global_sound(&events, &self.global_sound);
        let note_behavior = apply_note_behavior(
            &events,
            &self.note_behaviors,
            self.part_index,
            &self.held_notes,
        );
        let NoteBehaviorResult { events, held_notes } = note_behavior;
        self.held_notes = held_notes;

        Ok(NativeTickResult {
            emitted_events: context.emitted_events,
            events: dedupe_simultaneous_notes(&events),
            mapped_intents: mapped.intents,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpretation::{
        AxisStrategy, InterpretationEventProfile, InterpretationStateProfile, TickStrategy,
    };
    use crate::mapping::default_mapping_config;
    use crate::transforms::{GlobalSoundConfig, VelocityCurve};

    #[test]
    fn ticks_life_behavior_end_to_end() {
        let mut engine = NativePartEngine::new(NativePartEngineConfig {
            behavior: NativeBehavior::Life,
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "menu_profile".into(),
                event: InterpretationEventProfile { enabled: true },
                state: InterpretationStateProfile {
                    enabled: true,
                    tick: TickStrategy::WholeGridTransitions,
                },
                x: AxisStrategy::ScaleStep { step: 1 },
                y: AxisStrategy::ScaleStep { step: 2 },
            },
            mapping_config: default_mapping_config(),
            global_sound: GlobalSoundConfig {
                velocity_scale_pct: 100,
                velocity_curve: VelocityCurve::Linear,
                note_length_ms: 120,
            },
            note_behaviors: vec![NoteBehavior::Oneshot; 16],
            part_index: 0,
        })
        .unwrap();

        engine
            .on_input(DeviceInput::GridPress { x: 2, y: 3 }, 120.0)
            .unwrap();
        engine
            .on_input(DeviceInput::GridPress { x: 3, y: 3 }, 120.0)
            .unwrap();
        engine
            .on_input(DeviceInput::GridPress { x: 4, y: 3 }, 120.0)
            .unwrap();

        let tick = engine.tick(120.0).unwrap();
        assert!(tick.model.cells[crate::grid_index(3, 2)]);
        assert!(!tick.events.is_empty());
    }

    #[test]
    fn scan_interpretation_advances_with_engine_ticks() {
        let mut engine = NativePartEngine::new(NativePartEngineConfig {
            behavior: NativeBehavior::Sequencer,
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "scan_profile".into(),
                event: InterpretationEventProfile { enabled: false },
                state: InterpretationStateProfile {
                    enabled: true,
                    tick: TickStrategy::ScanRowActive {
                        sections: None,
                        reverse: false,
                    },
                },
                x: AxisStrategy::ScaleStep { step: 1 },
                y: AxisStrategy::ScaleStep { step: 2 },
            },
            mapping_config: default_mapping_config(),
            global_sound: GlobalSoundConfig {
                velocity_scale_pct: 100,
                velocity_curve: VelocityCurve::Linear,
                note_length_ms: 120,
            },
            note_behaviors: vec![NoteBehavior::Oneshot; 16],
            part_index: 0,
        })
        .unwrap();
        engine
            .on_input(DeviceInput::GridPress { x: 0, y: 1 }, 120.0)
            .unwrap();

        let first = engine.tick(120.0).unwrap();
        let second = engine.tick(120.0).unwrap();

        assert!(first.events.is_empty());
        assert!(!second.events.is_empty());
    }
}
