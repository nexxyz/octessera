use super::*;

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    let mut snapshot = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        if metadata.is_dir() {
            snapshot.push((path.strip_prefix(root).unwrap().to_path_buf(), None));
            snapshot.extend(
                snapshot_tree(&path)
                    .into_iter()
                    .map(|(child, bytes)| (path.strip_prefix(root).unwrap().join(child), bytes)),
            );
        } else {
            snapshot.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                Some(fs::read(path).unwrap()),
            ));
        }
    }
    snapshot.sort_by(|left, right| left.0.cmp(&right.0));
    snapshot
}

#[test]
fn restore_rejects_invalid_target_stores_without_mutation() {
    for invalid in [
        "legacy-mixed-only",
        "system-only",
        "patch-only",
        "malformed-system",
        "malformed-patch",
    ] {
        let fixture = Fixture::new(invalid);
        match invalid {
            "legacy-mixed-only" => {
                fs::remove_file(fixture.store.join("system.json")).unwrap();
                fs::remove_file(crate::platform_service::default_patch_path(&fixture.store))
                    .unwrap();
                fs::remove_file(crate::platform_service::current_patch_path(&fixture.store))
                    .unwrap();
                fs::write(
                    fixture.store.join("default.json"),
                    serde_json::to_vec(&user_data_archive::canonical_defaults()).unwrap(),
                )
                .unwrap();
            }
            "system-only" => {
                fs::remove_file(crate::platform_service::default_patch_path(&fixture.store))
                    .unwrap();
            }
            "patch-only" => {
                fs::remove_file(fixture.store.join("system.json")).unwrap();
            }
            "malformed-system" => {
                fs::write(fixture.store.join("system.json"), b"{").unwrap();
            }
            "malformed-patch" => {
                fs::write(
                    crate::platform_service::default_patch_path(&fixture.store),
                    b"{",
                )
                .unwrap();
            }
            _ => unreachable!(),
        }

        let original = snapshot_tree(&fixture.root);
        let session = format!("invalid-{invalid}");
        assert!(fixture.restore(&session, false).is_err(), "{invalid}");
        assert_eq!(snapshot_tree(&fixture.root), original, "{invalid}");
        assert!(
            !fixture
                .staging
                .join(format!("octessera-pre-restore-{session}.oct"))
                .exists(),
            "{invalid}"
        );
    }
}

#[test]
fn a_damaged_preset_the_pre_restore_archive_skips_is_kept_raw_in_backups() {
    let fixture = Fixture::new("damaged-preset");
    let damaged = crate::platform_service::preset_patch_path(&fixture.store, "Broken").unwrap();
    fs::write(&damaged, b"{ not a patch").unwrap();
    let current = fs::read(crate::platform_service::current_patch_path(&fixture.store)).unwrap();

    fixture.restore("damaged-preset", false).unwrap();

    let kept = fixture.store.join("backups/pre-restore-damaged-preset");
    assert_eq!(
        fs::read(kept.join("patches/Broken.json")).unwrap(),
        b"{ not a patch"
    );
    assert_eq!(fs::read(kept.join("current.json")).unwrap(), current);
    assert!(!damaged.exists());
    assert_eq!(
        fs::read(fixture.store.join("backups/live.json")).unwrap(),
        b"live backup"
    );

    fixture.restore("after-damaged-preset", false).unwrap();
    assert!(kept.join("patches/Broken.json").is_file());
    assert!(!fixture
        .store
        .join("backups/pre-restore-after-damaged-preset")
        .exists());
}
