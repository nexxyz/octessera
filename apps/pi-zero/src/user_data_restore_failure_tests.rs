use super::*;
use playback_runtime::{
    new_user_data_bundle, UserDataBundleMetadata, UserDataMusicalState, UserPreferenceDelta,
};
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
        rollback_tree(current, old, replacement);
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
