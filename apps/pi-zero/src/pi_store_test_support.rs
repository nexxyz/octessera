use playback_runtime::{split_system_patch_documents, SystemPatchDocuments};
use serde_json::Value;
use std::path::Path;

pub(crate) fn write_pair(store_dir: &Path, full_config: &Value) -> SystemPatchDocuments {
    let documents = split_system_patch_documents(full_config).unwrap();
    std::fs::create_dir_all(store_dir).unwrap();
    std::fs::write(
        store_dir.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    std::fs::write(
        store_dir.join("default.patch.json"),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    documents
}
