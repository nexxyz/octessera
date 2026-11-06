//! End-to-end scenarios: real device inputs through the desktop runtime, host adapter, sample
//! preparation and engine, rendered headlessly in virtual time (no window, no sound card).
//! Run with `corepack pnpm run e2e`.
mod behaviors;
mod default_patch;
mod device_menu;
mod follow_transpose;
mod gestures;
mod instruments;
mod link_mapping;
mod midi;
mod performance_pass;
mod play_fx;
mod preset_loads;
mod recording;
mod save_reload;
mod sequencer_pages;
mod soak;
mod support;
