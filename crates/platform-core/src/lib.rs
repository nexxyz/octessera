mod behavior;
mod behaviors;
mod engine;
mod events;
mod grid;
mod interpretation;
mod mapping;
mod transforms;

pub use behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorEngine, BehaviorRenderModel, CellTriggerType, DeviceInput, GridInteraction,
};
pub use behaviors::{
    get_native_behavior, list_native_behavior_ids, NativeBehavior, NativeBehaviorState,
};
pub use engine::{NativeInputResult, NativePartEngine, NativePartEngineConfig, NativeTickResult};
pub use events::MusicalEvent;
pub use grid::{grid_index, GridDimensions, GRID_HEIGHT, GRID_WIDTH};
pub use interpretation::{
    extract_transitions, interpret_grid, AxisStrategy, CellTransition, CellTransitionKind,
    CellTriggerIntent, CellTriggerKind, GridSnapshot, InterpretationEventProfile,
    InterpretationProfile, InterpretationStateProfile, TickStrategy,
};
pub use mapping::{
    default_mapping_config, map_intents_to_musical_events, MappingConfig, MappingResult, RangeMode,
    TriggerAction, TriggerTarget,
};
pub use transforms::{
    apply_global_sound, apply_note_behavior, dedupe_simultaneous_notes, GlobalSoundConfig,
    NoteBehavior, NoteBehaviorResult, VelocityCurve,
};
