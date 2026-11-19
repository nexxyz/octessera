use realtime_engine::synth::MixerRoute;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) buses: Option<Vec<FxBusDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) master: Option<MasterMixerDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxBusDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slot1: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slot2: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slot3: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pan_pos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) volume_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) auto_name: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MasterMixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) slots: Option<Vec<FxSlotDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxSlotDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentMixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) pan_pos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(in crate::native_runner) route: Option<MixerRoute>,
}
