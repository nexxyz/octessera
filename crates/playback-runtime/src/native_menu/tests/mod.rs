use super::*;
use platform_core::AUX_ENCODER_COUNT;
use platform_core::LAYER_COUNT;

mod drum;
mod dsp_mode;
mod duck_ranges;
mod fixture_tests;
mod fixtures;
mod fm;
mod help;
mod help_enum_tests;
mod link;
mod note_set_help;
mod play;
mod pluck;
mod root;
mod sampler_bindings;
mod timing;
mod voice;

pub(super) use fixture_tests::*;
pub(super) use fixtures::*;
