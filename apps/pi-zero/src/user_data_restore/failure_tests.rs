use super::*;
use playback_runtime::{
    new_user_data_bundle, UserDataBundleMetadata, UserDataMusicalState, UserPreferenceDelta,
};
use serde_json::{json, Value};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod mount_tests;
#[cfg(test)]
mod store_validation_tests;
#[cfg(test)]
mod swap_tests;

struct Fixture {
    root: PathBuf,
    store: PathBuf,
    samples: PathBuf,
    recordings: PathBuf,
    screen_recordings: PathBuf,
    staging: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = temp_root(name);
        let store = root.join("store");
        let samples = root.join("samples");
        let recordings = root.join("recordings");
        let screen_recordings = root.join("screen-recordings");
        let staging = root.join(STAGING_DIR_NAME);
        fs::create_dir_all(store.join("patches")).unwrap();
        for path in [&samples, &recordings, &screen_recordings] {
            fs::create_dir_all(path).unwrap();
        }
        let defaults = user_data_archive::canonical_defaults();
        let documents = playback_runtime::split_system_patch_documents(&defaults).unwrap();
        fs::write(
            store.join("system.json"),
            serde_json::to_vec(&documents.system).unwrap(),
        )
        .unwrap();
        fs::write(
            crate::platform_service::default_patch_path(&store),
            serde_json::to_vec(&documents.patch).unwrap(),
        )
        .unwrap();
        fs::write(
            crate::platform_service::current_patch_path(&store),
            serde_json::to_vec(&documents.patch).unwrap(),
        )
        .unwrap();
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
            staging,
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
            &LiveFilesystems,
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
        .staging
        .join("octessera-pre-restore-backup-failure.oct");
    fs::create_dir_all(&backup_path).unwrap();

    assert!(fixture.restore("backup-failure", true).is_err());
    fixture.assert_live_data();
    assert!(backup_path.is_dir());
}

#[test]
fn restore_writes_only_into_its_staging_folder() {
    let fixture = Fixture::new("staging-only");
    let names = |path: &Path| {
        let mut names: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    };
    let mut before = names(&fixture.root);
    before.push(STAGING_DIR_NAME.to_string());
    before.sort();

    fixture.restore("staging-only", true).unwrap();

    assert_eq!(names(&fixture.root), before);
    assert_eq!(
        names(&fixture.staging),
        ["octessera-pre-restore-staging-only.oct"]
    );
}

#[test]
fn a_restore_keeps_only_the_two_newest_pre_restore_archives() {
    let fixture = Fixture::new("archive-prune");
    fs::create_dir_all(&fixture.staging).unwrap();
    let now = std::time::SystemTime::now();
    for (age_hours, session) in [(3, "oldest"), (2, "older"), (1, "old")] {
        let path = fixture
            .staging
            .join(format!("octessera-pre-restore-{session}.oct"));
        fs::write(&path, b"archive").unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(now - std::time::Duration::from_secs(age_hours * 3600))
            .unwrap();
    }
    fs::write(fixture.staging.join("notes.txt"), b"not an archive").unwrap();

    fixture.restore("newest", true).unwrap();

    let mut names: Vec<_> = fs::read_dir(&fixture.staging)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "notes.txt",
            "octessera-pre-restore-newest.oct",
            "octessera-pre-restore-old.oct"
        ]
    );
}

#[test]
fn a_media_restore_keeps_the_bundled_library_at_its_paths() {
    let fixture = Fixture::new("library-paths");
    let kick = fixture.samples.join("Drum/kick/Kick2.wav");
    let vocal = fixture.samples.join("Vocal/Word_Up.wav");
    for (path, bytes) in [(&kick, b"kick"), (&vocal, b"word")] {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fixture.restore("library-paths", true).unwrap();

    assert_eq!(fs::read(&kick).unwrap(), b"kick");
    assert_eq!(fs::read(&vocal).unwrap(), b"word");
    assert!(!fixture.samples.join("Drum/Drum").exists());
    assert!(!fixture.samples.join("Vocal/Vocal").exists());
}

#[test]
fn recovery_path_collision_keeps_the_new_pre_restore_archive() {
    let fixture = Fixture::new("recovery-collision");
    let old_path = fixture
        .staging
        .join(".octessera-store-old-recovery-collision");
    fs::create_dir_all(&old_path).unwrap();
    fs::write(old_path.join("recovery-marker"), b"keep me").unwrap();

    assert!(fixture.restore("recovery-collision", true).is_err());
    fixture.assert_live_data();
    assert_eq!(
        fs::read(old_path.join("recovery-marker")).unwrap(),
        b"keep me"
    );
    assert!(fixture
        .staging
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
        move_in_failure_at: Some((1, 0)),
        ..FaultInjection::default()
    };

    let result = replace_trees_with_faults(&LiveFilesystems, &refs, &trees[1].0, &mut faults);
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
        move_in_failure_at: Some((0, 0)),
        rollback_failure_at: Some(0),
        ..FaultInjection::default()
    };

    let result =
        replace_trees_with_faults(&LiveFilesystems, &refs, &trees[1].0, &mut faults).unwrap_err();
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
        move_in_failure_at: Some((1, 0)),
        post_swap_rollback_failure_at: Some(0),
        ..FaultInjection::default()
    };

    let result =
        replace_trees_with_faults(&LiveFilesystems, &refs, &trees[1].0, &mut faults).unwrap_err();
    assert!(result.contains("rollback failed"));
    assert!(trees[0].1.is_dir());
    assert!(trees[0].2.is_dir());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restore_rehydrates_canonical_patch_preferences_and_patch_aux_bindings() {
    let fixture = Fixture::new("rehydrate-runtime-owned-config");
    let mut staged = fixture.staged(false);
    staged
        .bundle
        .preferences
        .values
        .insert("displayBrightness".into(), json!(6));
    staged.bundle.preferences.values.insert(
        "audioOutputs".into(),
        json!({ "dac": false, "usb": true, "hdmi": false }),
    );
    let mut current_patch = staged.bundle.current_state.patch.clone();
    current_patch["runtimeConfig"]["linkLfos"][0] = json!({
        "enabled": true,
        "target": { "key": "instruments.0.mixer.volume", "kind": "number" },
        "period": "1/4",
        "depthPct": 37
    });
    current_patch["runtimeConfig"]["auxBindings"] = json!({
        "aux1": {
            "turnKey": "sound.noteLengthMs",
            "pressAction": { "kind": "behavior_action", "actionType": "clear" , "layer": 0 }
        },
        "aux2": {
            "turnKey": "displayBrightness",
            "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
        }
    });
    current_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"] =
        json!([{ "level": null, "sampleSlot": 3, "x": 2, "y": 4 }]);
    staged.bundle.current_state.patch = current_patch;
    let mut default_patch = staged.bundle.default_state.patch.clone();
    default_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"] =
        json!([{ "level": null, "sampleSlot": 5, "x": 6, "y": 1 }]);
    staged.bundle.default_state.patch = default_patch;

    super::restore(
        &LiveFilesystems,
        &fixture.store,
        &fixture.samples,
        &fixture.recordings,
        &fixture.screen_recordings,
        "rehydrate-runtime-owned-config",
        staged,
    )
    .unwrap();

    let system: Value =
        serde_json::from_slice(&fs::read(fixture.store.join("system.json")).unwrap()).unwrap();
    let current: Value = serde_json::from_slice(
        &fs::read(crate::platform_service::current_patch_path(&fixture.store)).unwrap(),
    )
    .unwrap();
    let default: Value = serde_json::from_slice(
        &fs::read(crate::platform_service::default_patch_path(&fixture.store)).unwrap(),
    )
    .unwrap();
    assert_eq!(system["runtimeConfig"]["displayBrightness"], 6);
    assert_eq!(
        system["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    assert_eq!(current["runtimeConfig"]["linkLfos"][0]["enabled"], true);
    assert_eq!(current["runtimeConfig"]["linkLfos"][0]["depthPct"], 37);
    assert_eq!(
        current["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "sound.noteLengthMs"
    );
    assert_eq!(
        current["runtimeConfig"]["auxBindings"]["aux2"]["turnKey"],
        "displayBrightness"
    );
    assert!(system["runtimeConfig"].get("auxBindings").is_none());
    assert_eq!(default["kind"], "octessera.patch");
    assert_eq!(
        current["runtimeConfig"]["instruments"][0]["sample"]["assignments"][0]["sampleSlot"],
        3
    );
    assert_eq!(
        default["runtimeConfig"]["instruments"][0]["sample"]["assignments"][0]["sampleSlot"],
        5
    );
}

#[test]
fn restore_preserves_target_host_role_and_keeps_backup_role_free() {
    let fixture = Fixture::new("host-role");
    let mut host = user_data_archive::canonical_defaults();
    host["runtimeConfig"]["audioOutputs"]["usb"] = json!(false);
    host["runtimeConfig"]["usb"]["dataRole"] = json!("host");
    host["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(false);
    host["runtimeConfig"]["midi"]["inId"] = json!("local-midi-in");
    host["runtimeConfig"]["midi"]["outId"] = json!("local-midi-out");
    let documents = playback_runtime::split_system_patch_documents(&host).unwrap();
    fs::write(
        fixture.store.join("system.json"),
        serde_json::to_vec(&documents.system).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::default_patch_path(&fixture.store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();
    fs::write(
        crate::platform_service::current_patch_path(&fixture.store),
        serde_json::to_vec(&documents.patch).unwrap(),
    )
    .unwrap();

    fixture.restore("host-role", false).unwrap();
    fixture.assert_live_media_data();

    let payload: Value =
        serde_json::from_slice(&fs::read(fixture.store.join("system.json")).unwrap()).unwrap();
    assert_eq!(payload["runtimeConfig"]["usb"]["dataRole"], "host");
    assert_eq!(payload["runtimeConfig"]["audioOutputs"]["usb"], false);
    assert_eq!(payload["runtimeConfig"]["usb"]["midiOutEnabled"], false);
    assert_eq!(payload["runtimeConfig"]["midi"]["inId"], "local-midi-in");
    assert_eq!(payload["runtimeConfig"]["midi"]["outId"], "local-midi-out");
    assert_eq!(
        payload["runtimeConfig"]["displayBrightness"],
        user_data_archive::canonical_defaults()["runtimeConfig"]["displayBrightness"]
    );
    let backup = fs::read(fixture.staging.join("octessera-pre-restore-host-role.oct")).unwrap();
    assert!(!backup
        .windows(b"dataRole".len())
        .any(|window| window == b"dataRole"));
}
