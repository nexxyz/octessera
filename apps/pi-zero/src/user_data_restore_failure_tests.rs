use super::*;
use playback_runtime::{
    new_user_data_bundle, UserDataBundleMetadata, UserDataMusicalState, UserPreferenceDelta,
};
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

struct Fixture {
    root: PathBuf,
    store: PathBuf,
    samples: PathBuf,
    recordings: PathBuf,
    screen_recordings: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = temp_root(name);
        let store = root.join("store");
        let samples = root.join("samples");
        let recordings = root.join("recordings");
        let screen_recordings = root.join("screen-recordings");
        fs::create_dir_all(store.join("patches")).unwrap();
        for path in [&samples, &recordings, &screen_recordings] {
            fs::create_dir_all(path).unwrap();
        }
        let defaults = user_data_archive::canonical_defaults();
        let defaults_bytes = serde_json::to_vec(&defaults).unwrap();
        fs::write(store.join("default.json"), &defaults_bytes).unwrap();
        fs::write(store.join("current.json"), defaults_bytes).unwrap();
        fs::write(samples.join("live-sample.wav"), b"live sample").unwrap();
        fs::write(recordings.join("live-take.wav"), b"live recording").unwrap();
        fs::write(
            screen_recordings.join("live-screen.webm"),
            b"live screen recording",
        )
        .unwrap();
        fs::write(store.join("device.json"), b"live device").unwrap();
        fs::write(store.join("live-marker.txt"), b"live store marker").unwrap();
        fs::create_dir_all(store.join("backups")).unwrap();
        fs::write(store.join("backups/live.json"), b"live backup").unwrap();
        Self {
            root,
            store,
            samples,
            recordings,
            screen_recordings,
        }
    }

    fn staged(&self, media_included: bool) -> StagedRestore {
        let canonical = user_data_archive::canonical_defaults();
        let bundle = new_user_data_bundle(
            UserDataBundleMetadata {
                board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
                runtime_version: env!("CARGO_PKG_VERSION").into(),
            },
            Vec::new(),
            UserDataMusicalState {
                patch: canonical.clone(),
            },
            UserDataMusicalState { patch: canonical },
            UserPreferenceDelta::empty(),
            media_included,
            Vec::new(),
            &user_data_archive::canonical_defaults(),
        )
        .unwrap();
        StagedRestore {
            bundle,
            media: Vec::new(),
            root: self.root.join("stage"),
        }
    }

    fn restore(&self, session: &str, media_included: bool) -> Result<(), String> {
        super::restore(
            &self.store,
            &self.samples,
            &self.recordings,
            &self.screen_recordings,
            session,
            self.staged(media_included),
        )
    }

    fn assert_live_data(&self) {
        self.assert_live_media_data();
        assert_eq!(
            fs::read(self.store.join("device.json")).unwrap(),
            b"live device"
        );
        assert_eq!(
            fs::read(self.store.join("live-marker.txt")).unwrap(),
            b"live store marker"
        );
    }

    fn assert_live_media_data(&self) {
        assert_eq!(
            fs::read(self.samples.join("live-sample.wav")).unwrap(),
            b"live sample"
        );
        assert_eq!(
            fs::read(self.recordings.join("live-take.wav")).unwrap(),
            b"live recording"
        );
        assert_eq!(
            fs::read(self.screen_recordings.join("live-screen.webm")).unwrap(),
            b"live screen recording"
        );
        assert_eq!(
            fs::read(self.store.join("backups/live.json")).unwrap(),
            b"live backup"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn temp_root(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "octessera-user-restore-{name}-{}-{nonce}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn symlink_dir(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

#[test]
fn pre_restore_backup_failure_leaves_live_store_and_samples_untouched() {
    let fixture = Fixture::new("backup-failure");
    let backup_path = fixture
        .root
        .join("octessera-pre-restore-backup-failure.oct");
    fs::create_dir(&backup_path).unwrap();

    assert!(fixture.restore("backup-failure", true).is_err());
    fixture.assert_live_data();
    assert!(backup_path.is_dir());
}

#[test]
fn recovery_path_collision_keeps_the_new_pre_restore_archive() {
    let fixture = Fixture::new("recovery-collision");
    let old_path = fixture.root.join(".octessera-store-old-recovery-collision");
    fs::create_dir(&old_path).unwrap();
    fs::write(old_path.join("recovery-marker"), b"keep me").unwrap();

    assert!(fixture.restore("recovery-collision", true).is_err());
    fixture.assert_live_data();
    assert_eq!(
        fs::read(old_path.join("recovery-marker")).unwrap(),
        b"keep me"
    );
    assert!(fixture
        .root
        .join("octessera-pre-restore-recovery-collision.oct")
        .is_file());
}

#[test]
fn protected_tree_symlink_fails_without_mutating_live_data() {
    let fixture = Fixture::new("protected-symlink");
    let target = fixture.root.join("outside");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("escaped.json"), b"outside").unwrap();
    symlink_dir(&target, &fixture.store.join("backups/escaped")).unwrap();

    assert!(fixture.restore("protected-symlink", true).is_err());
    fixture.assert_live_data();
    assert!(fs::symlink_metadata(fixture.store.join("backups/escaped"))
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn unsafe_protected_entry_fails_without_mutating_live_data() {
    let fixture = Fixture::new("protected-entry");
    fs::remove_file(fixture.store.join("device.json")).unwrap();
    fs::create_dir(fixture.store.join("device.json")).unwrap();
    fs::write(fixture.store.join("device.json/marker"), b"unsafe entry").unwrap();

    assert!(fixture.restore("protected-entry", true).is_err());
    fixture.assert_live_media_data();
    assert!(fixture.store.join("device.json").is_dir());
    assert_eq!(
        fs::read(fixture.store.join("device.json/marker")).unwrap(),
        b"unsafe entry"
    );
}

#[test]
fn swap_failure_rolls_back_an_already_swapped_tree() {
    let root = temp_root("swap-failure");
    let current_store = root.join("store");
    let replacement_store = root.join("store-new");
    let old_store = root.join("store-old");
    let current_samples = root.join("samples");
    let replacement_samples = root.join("samples-new");
    let old_samples = root.join("samples-old");
    fs::create_dir_all(&current_store).unwrap();
    fs::create_dir_all(&replacement_store).unwrap();
    fs::create_dir_all(&current_samples).unwrap();
    fs::write(current_store.join("state"), b"live store").unwrap();
    fs::write(replacement_store.join("state"), b"new store").unwrap();
    fs::write(current_samples.join("state"), b"live samples").unwrap();

    let trees = [
        (
            current_store.as_path(),
            replacement_store.as_path(),
            old_store.as_path(),
        ),
        (
            current_samples.as_path(),
            replacement_samples.as_path(),
            old_samples.as_path(),
        ),
    ];
    assert!(swap_tree(trees[0].0, trees[0].1, trees[0].2).is_ok());
    assert_eq!(fs::read(current_store.join("state")).unwrap(), b"new store");

    let result = swap_tree(trees[1].0, trees[1].1, trees[1].2);
    assert!(result.is_err());
    for (current, replacement, old) in trees[..1].iter().rev() {
        rollback_tree(current, old, replacement).unwrap();
    }

    assert_eq!(
        fs::read(current_store.join("state")).unwrap(),
        b"live store"
    );
    assert_eq!(
        fs::read(current_samples.join("state")).unwrap(),
        b"live samples"
    );
    assert_eq!(
        fs::read(replacement_store.join("state")).unwrap(),
        b"new store"
    );
    assert!(!old_store.exists());
    assert!(!old_samples.exists());
    let _ = fs::remove_dir_all(root);
}

fn transaction_fixture(name: &str) -> (PathBuf, Vec<(PathBuf, PathBuf, PathBuf)>) {
    let root = temp_root(name);
    let mut trees = Vec::new();
    for name in ["store", "samples"] {
        let current = root.join(name);
        let replacement = root.join(format!("{name}-new"));
        let old = root.join(format!("{name}-old"));
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&replacement).unwrap();
        fs::write(current.join("state"), format!("live {name}")).unwrap();
        fs::write(replacement.join("state"), format!("new {name}")).unwrap();
        trees.push((current, replacement, old));
    }
    (root, trees)
}

fn tree_refs(trees: &[(PathBuf, PathBuf, PathBuf)]) -> Vec<(&Path, &Path, &Path)> {
    trees
        .iter()
        .map(|(current, replacement, old)| {
            (current.as_path(), replacement.as_path(), old.as_path())
        })
        .collect()
}

#[test]
fn injected_replacement_failure_preserves_recovery_tree_after_verified_rollback() {
    let (root, trees) = transaction_fixture("replacement-failure");
    let refs = tree_refs(&trees);
    let mut faults = FaultInjection {
        replacement_failure_at: Some(1),
        ..FaultInjection::default()
    };

    let result = replace_trees_with_faults(&refs, &mut faults);
    assert!(result.is_err());
    assert_eq!(fs::read(trees[0].0.join("state")).unwrap(), b"live store");
    assert_eq!(fs::read(trees[1].0.join("state")).unwrap(), b"live samples");
    assert!(trees[0].1.is_dir());
    assert!(trees[1].1.is_dir());
    assert!(!trees[0].2.exists());
    assert!(!trees[1].2.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn injected_rollback_failure_propagates_and_preserves_old_and_new_trees() {
    let (root, trees) = transaction_fixture("rollback-failure");
    let refs = tree_refs(&trees);
    let mut faults = FaultInjection {
        replacement_failure_at: Some(0),
        rollback_failure_at: Some(0),
        ..FaultInjection::default()
    };

    let result = replace_trees_with_faults(&refs, &mut faults).unwrap_err();
    assert!(result.contains("rollback failed"));
    assert!(trees[0].2.is_dir());
    assert!(trees[0].1.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn injected_post_swap_rollback_failure_propagates_and_preserves_recovery_trees() {
    let (root, trees) = transaction_fixture("post-swap-rollback-failure");
    let refs = tree_refs(&trees);
    let mut faults = FaultInjection {
        replacement_failure_at: Some(1),
        post_swap_rollback_failure_at: Some(0),
        ..FaultInjection::default()
    };

    let result = replace_trees_with_faults(&refs, &mut faults).unwrap_err();
    assert!(result.contains("rollback failed"));
    assert!(trees[0].1.is_dir());
    assert!(trees[0].2.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restore_rehydrates_migrated_patch_preferences_and_split_aux_ownership() {
    let fixture = Fixture::new("rehydrate-runtime-owned-config");
    let canonical = user_data_archive::canonical_defaults();
    let mut staged = fixture.staged(false);
    staged
        .bundle
        .preferences
        .values
        .insert("displayBrightness".into(), json!(42));
    staged.bundle.preferences.values.insert(
        "audioOutputs".into(),
        json!({ "dac": false, "usb": true, "hdmi": false }),
    );
    staged.bundle.current_state.patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 1,
        "runtimeConfig": {
            "masterVolume": 1,
            "layers": [{
                "worlds": { "behaviorId": "sequencer" },
                "linkLfo": {
                    "enabled": true,
                    "target": { "key": "instruments.0.mixer.volume", "kind": "number" },
                    "period": "1/4",
                    "depthPct": 37
                }
            }],
            "auxBindings": {
                "aux1": {
                    "turnKey": "sound.noteLengthMs",
                    "pressAction": { "kind": "behavior_action", "actionType": "clear" }
                },
                "aux2": {
                    "turnKey": "displayBrightness",
                    "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
                }
            }
        }
    });
    staged.bundle.default_state.patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": {}
    });

    super::restore(
        &fixture.store,
        &fixture.samples,
        &fixture.recordings,
        &fixture.screen_recordings,
        "rehydrate-runtime-owned-config",
        staged,
    )
    .unwrap();

    let current: Value =
        serde_json::from_slice(&fs::read(fixture.store.join("current.json")).unwrap()).unwrap();
    let default: Value =
        serde_json::from_slice(&fs::read(fixture.store.join("default.json")).unwrap()).unwrap();
    assert_eq!(current["runtimeConfig"]["displayBrightness"], 42);
    assert_eq!(
        current["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    assert_eq!(
        current["runtimeConfig"]["masterVolume"],
        canonical["runtimeConfig"]["masterVolume"]
    );
    assert_eq!(current["runtimeConfig"]["linkLfos"][0]["enabled"], true);
    assert_eq!(current["runtimeConfig"]["linkLfos"][0]["depthPct"], 37);
    assert_eq!(
        current["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "sound.noteLengthMs"
    );
    assert_eq!(
        current["runtimeConfig"]["auxBindings"]["aux2"],
        canonical["runtimeConfig"]["auxBindings"]["aux2"]
    );
    let mut expected_default = canonical.clone();
    expected_default["runtimeConfig"]["displayBrightness"] = json!(42);
    expected_default["runtimeConfig"]["audioOutputs"] =
        json!({ "dac": false, "usb": true, "hdmi": false });
    assert_eq!(default, expected_default);
}
