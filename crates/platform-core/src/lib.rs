mod behavior;
mod behaviors;
mod engine;
mod events;
mod grid;
mod interpretation;
mod interpretation_scan;
mod mapping;
pub mod palette;
mod platform_capabilities;
mod transforms;

pub use behavior::{
    BehaviorActionInput, BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext,
    BehaviorEngine, BehaviorRenderModel, BehaviorRenderPalette, CellTriggerType, DeviceInput,
    GridInteraction,
};
pub use behaviors::{
    behavior_catalog, behavior_categories, get_native_behavior, list_native_behavior_ids,
    BehaviorCatalogEntry, BehaviorCategory, NativeBehavior, NativeBehaviorState,
};
pub use engine::{NativeInputResult, NativeLayerEngine, NativeLayerEngineConfig, NativeTickResult};
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
pub use platform_capabilities::{
    PlatformCapabilities, AUX_ENCODER_COUNT, BUS_COUNT, BUS_FX_WARNING_SLOT_COUNT,
    GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT, LAYER_COUNT, OLED_HEIGHT, OLED_WIDTH,
    PAN_POSITION_COUNT, PLATFORM_CAPABILITIES, SAMPLE_SLOT_COUNT, SCAN_SECTION_COUNTS,
    SPARKS_FX_MAX_CONCURRENT,
};
pub use transforms::{
    apply_global_sound, apply_note_behavior, dedupe_simultaneous_notes, GlobalSoundConfig,
    NoteBehavior, NoteBehaviorResult, VelocityCurve,
};
