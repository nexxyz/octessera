use super::{pct_param, GainRamp, ENVELOPE_MS};
use crate::synth::engine::support::param_f32;
use crate::synth::fx::{fx_bus_state_from_params, process_fx_bus_slot, FxBusState};
use crate::synth::fx_params::FxBusParams;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::synth::engine) struct CrushParams {
    pub(in crate::synth::engine) bits: u32,
    pub(in crate::synth::engine) rate_div: u32,
    pub(in crate::synth::engine) mix: f32,
}

impl CrushParams {
    pub(super) fn from_map(params: &BTreeMap<String, Value>) -> Self {
        Self {
            bits: param_f32(params, "bits", 6.0).round().clamp(1.0, 16.0) as u32,
            rate_div: param_f32(params, "rateDiv", 6.0).round().clamp(1.0, 32.0) as u32,
            mix: pct_param(params, "mixPct", 100.0),
        }
    }

    fn line(self, mix: f32) -> FxBusParams {
        FxBusParams::Bitcrusher {
            rate_div: self.rate_div,
            bits: self.bits,
            mix,
        }
    }
}

/// The bus bitcrusher on each channel, its mix eased in and out over a few milliseconds.
#[derive(Clone)]
pub(in crate::synth::engine) struct CrushVoice {
    pub(super) params: CrushParams,
    crush_l: FxBusState,
    crush_r: FxBusState,
    pub(super) envelope: GainRamp,
    releasing: bool,
    sample_rate: u32,
}

impl CrushVoice {
    pub(super) fn new(params: CrushParams, sample_rate: u32) -> Self {
        let line = params.line(0.0);
        Self {
            params,
            crush_l: fx_bus_state_from_params(&line, sample_rate),
            crush_r: fx_bus_state_from_params(&line, sample_rate),
            envelope: GainRamp::new(0.0, ENVELOPE_MS, sample_rate),
            releasing: false,
            sample_rate,
        }
    }

    pub(super) fn release(&mut self) {
        self.releasing = true;
    }

    pub(super) fn finished(&self) -> bool {
        self.releasing && self.envelope.value() <= 0.0
    }

    pub(super) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let envelope = self.envelope.toward(if self.releasing { 0.0 } else { 1.0 });
        let line = self.params.line(self.params.mix * envelope);
        (
            process_fx_bus_slot(&line, &mut self.crush_l, left, 0.0, self.sample_rate),
            process_fx_bus_slot(&line, &mut self.crush_r, right, 0.0, self.sample_rate),
        )
    }
}
