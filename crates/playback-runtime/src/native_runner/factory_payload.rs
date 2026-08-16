use super::*;

pub(super) fn native_factory_payload() -> Value {
    serde_json::from_str(include_str!("../../../../config/defaults/base.json"))
        .expect("checked-in base default config is valid JSON")
}
