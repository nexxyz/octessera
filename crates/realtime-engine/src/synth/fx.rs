use super::fx_params::{DuckSource, FilterLfoKind, FxBusParams};
use super::runtime_state::*;
use super::types::*;
use std::f32::consts::PI;

mod algorithms;
mod bus;
mod master;
mod state;

pub(super) use bus::process_fx_bus_slot;
pub(super) use master::process_master_fx_slot;
pub(super) use state::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, FxBusState, MasterFxState,
};
