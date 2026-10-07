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
mod native_behavior_transport;
mod native_impl;
mod pattern_music;
mod play;

#[cfg(test)]
mod liveness_probe_tests;
#[cfg(test)]
mod native_behavior_transport_tests;
#[cfg(test)]
mod palette_tests;
#[cfg(test)]
mod pattern_music_tests;
#[cfg(test)]
mod tests;

pub use catalog::{behavior_catalog, behavior_categories, BehaviorCatalogEntry, BehaviorCategory};
pub use native_behavior::{
    get_native_behavior, list_native_behavior_ids, NativeBehavior, NativeBehaviorState,
};
pub use pattern_music::PatternBehaviorState;
