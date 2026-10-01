use super::{
    compose_local_system_patch_documents, split_local_system_patch_documents, ConfigDto,
    PreparedConfigPayload, Value,
};

pub(super) fn prepare_local_patch_payload(
    patch: Value,
    current: &Value,
) -> Result<PreparedConfigPayload, String> {
    let system = split_local_system_patch_documents(current)?.system;
    let payload = compose_local_system_patch_documents(&system, &patch)?;
    let envelope = ConfigDto::decode(&payload)?;
    Ok(PreparedConfigPayload {
        apply_payload: payload.clone(),
        payload,
        envelope,
        source_revision: None,
    })
}

#[cfg(test)]
#[path = "local_patch_document_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "local_patch_completeness_tests.rs"]
mod completeness_tests;
