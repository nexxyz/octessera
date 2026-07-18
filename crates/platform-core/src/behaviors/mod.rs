mod behavior_config;
mod behavior_native_lifecycle;
mod catalog;
mod cellular;
mod fields;
mod geometry;
mod growth;
mod motion;
mod native_behavior;
mod native_behavior_dispatch;
mod native_behavior_render;
mod native_behavior_serialize;
mod native_behavior_tick;
mod native_impl;
mod pattern_music;
mod play;

#[cfg(test)]
mod liveness_probe_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod tests;

pub use catalog::{behavior_catalog, behavior_categories, BehaviorCatalogEntry, BehaviorCategory};
#[allow(unused_imports)]
pub use cellular::LifeState;
pub use native_behavior::{
    get_native_behavior, list_native_behavior_ids, NativeBehavior, NativeBehaviorState,
};
#[allow(unused_imports)]
pub use native_impl::{
    AntState, BoidsState, BounceState, BrainState, BubblesState, CoralState, CracksState,
    CrystalGrowthState, CyclicState, DlaState, ForestFireState, FractalExplorerState, GravityState,
    InkState, IsingState, KeysState, KuramotoState, LavaLampState, LightningState, LooperState,
    MazeGrowthState, OrbitState, PhysarumState, PredatorPreyState, RaindropsState,
    ReactionDiffusionState, RiversState, SandRipplesState, ShapesState, VinesState, WaveState,
};
pub use pattern_music::PatternBehaviorState;
#[allow(unused_imports)]
pub use play::{NoneState, SequencerState};
