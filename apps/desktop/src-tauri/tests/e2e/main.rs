//! End-to-end scenarios: real device inputs through the desktop runtime, host adapter, sample
//! preparation and engine, rendered headlessly in virtual time (no window, no sound card).
//! Run with `corepack pnpm run e2e`.
mod default_patch;
mod gestures;
mod instruments;
mod sequencer_pages;
mod support;
mod teaser;
mod teaser_take;
