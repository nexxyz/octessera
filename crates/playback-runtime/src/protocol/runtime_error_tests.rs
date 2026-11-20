use super::*;
use serde_json::json;

#[test]
fn a_failed_preset_save_carries_the_preset_name_into_its_error_facts() {
    let save = RuntimePlatformEffect::StoreSavePreset {
        name: "Pad Wash".into(),
        payload: json!({}),
        mode: None,
    };
    let facts = save.failure_facts("disk full".into());
    assert_eq!(facts.preset_name.as_deref(), Some("Pad Wash"));
    assert_eq!(
        facts
            .into_metadata(RuntimeRecovery::RetainLastGood)
            .preset_name
            .as_deref(),
        Some("Pad Wash")
    );
    assert!(RuntimePlatformEffect::StoreListPresets
        .failure_facts("x".into())
        .preset_name
        .is_none());
}
