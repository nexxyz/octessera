use super::*;

pub(super) fn native_factory_payload() -> Value {
    serde_json::from_str(include_str!("../../../../config/defaults/base.json"))
        .expect("checked-in base default config is valid JSON")
}

#[cfg(test)]
pub(super) fn native_factory_payload_at_revision(revision: u64) -> Value {
    let mut payload = native_factory_payload();
    payload["revision"] = revision.into();
    payload
}
