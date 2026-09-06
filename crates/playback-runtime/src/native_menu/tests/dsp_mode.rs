use super::*;

#[test]
fn dsp_mode_help_covers_both_capability_values() {
    let mut config = config();
    config.audio_optimization_capacity_available = true;
    let target = NativeMenuModel::new(config)
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:sound.optimizeFor")
        .expect("DSP mode help target");
    let entry = crate::native_help::resolve_native_help_entry(&target).expect("DSP mode help");
    let copy = format!("{} {}", entry.line1, entry.line2);
    assert_eq!(entry.title, "DSP Mode");
    assert!(copy.contains("Inline / low latency"));
    assert!(copy.contains("Multicore / capacity"));
}

#[test]
fn jack_help_target_matches_desktop_visibility_policy() {
    let desktop_target = NativeMenuModel::new(config())
        .help_targets()
        .into_iter()
        .find(|target| target.key == "key:audioOutputs.dac")
        .expect("desktop Jack Audio help target");
    assert_eq!(
        crate::native_help::resolve_native_help_entry(&desktop_target)
            .expect("desktop Jack Audio help entry")
            .title,
        "Jack Audio"
    );

    let mut pi_config = config();
    pi_config.jack_audio_required = true;
    assert!(!NativeMenuModel::new(pi_config)
        .help_targets()
        .into_iter()
        .any(|target| target.key == "key:audioOutputs.dac"));
}
