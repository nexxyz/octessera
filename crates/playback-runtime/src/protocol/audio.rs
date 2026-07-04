use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeMomentaryFxTarget {
    Global,
    FxBus { index: usize },
    Instrument { index: usize },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeAudioCommand {
    SetAudioConfig {
        revision: u64,
        config: Value,
    },
    SetMasterVolume {
        #[serde(rename = "volumePct")]
        volume_pct: f32,
    },
    SetInstrumentMixer {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(default, rename = "volumePct")]
        volume_pct: Option<f32>,
        #[serde(default, rename = "panPos")]
        pan_pos: Option<usize>,
    },
    SetInstrumentSlot {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        config: Value,
    },
    SetFxBusMixer {
        #[serde(rename = "busIndex")]
        bus_index: usize,
        #[serde(default, rename = "panPos")]
        pan_pos: Option<usize>,
    },
    SetSynthParam {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetSampleBankParam {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        path: String,
        value: f32,
    },
    SetFxBusSlot {
        #[serde(rename = "busIndex")]
        bus_index: usize,
        #[serde(rename = "slotIndex")]
        slot_index: usize,
        #[serde(rename = "fxType")]
        fx_type: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    SetGlobalFxSlot {
        #[serde(rename = "slotIndex")]
        slot_index: usize,
        #[serde(rename = "fxType")]
        fx_type: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStart {
        id: String,
        #[serde(rename = "fxType")]
        fx_type: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
        target: RuntimeMomentaryFxTarget,
    },
    MomentaryFxUpdate {
        id: String,
        #[serde(default)]
        params: BTreeMap<String, Value>,
    },
    MomentaryFxStop {
        id: String,
    },
    SamplePreview {
        #[serde(rename = "instrumentSlot")]
        instrument_slot: usize,
        #[serde(rename = "sampleSlot")]
        sample_slot: usize,
        path: String,
        velocity: u8,
    },
}
