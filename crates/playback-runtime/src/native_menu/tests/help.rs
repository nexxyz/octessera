use super::*;

#[test]
fn native_menu_help_targets_resolve_to_specific_tsv_rows() {
    let mut targets = Vec::new();
    let mut missing = Vec::new();
    for config in representative_help_configs() {
        let menu = NativeMenuModel::new(config);
        targets.extend(
            menu.help_targets()
                .into_iter()
                .filter(|target| target.kind != "action" || !target.key.is_empty()),
        );
    }
    targets.sort_by(|a, b| (&a.kind, &a.key, &a.path).cmp(&(&b.kind, &b.key, &b.path)));
    targets.dedup_by(|a, b| a.kind == b.kind && a.key == b.key && a.path == b.path);
    missing.extend(
        targets
            .into_iter()
            .filter(|target| crate::native_help::resolve_native_help_entry(target).is_none())
            .map(|target| format!("{} {} {}", target.kind, target.key, target.path)),
    );
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "missing help entries: {missing:#?}");
}
