use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) buses: Option<Vec<FxBusDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) master: Option<MasterMixerDto>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxBusDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slot1: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slot2: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slot3: Option<FxSlotDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pan_pos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) volume_pct: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) auto_name: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MasterMixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) slots: Option<Vec<FxSlotDto>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FxSlotDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentMixerDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) volume: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) pan_pos: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) route: Option<String>,
}
