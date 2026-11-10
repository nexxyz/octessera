use super::super::filesystem::{remove_tree_within, Filesystems};
use super::*;

/// Puts everything at or under one folder on its own filesystem, like the SD card mount.
struct MountedAt(PathBuf);

impl Filesystems for MountedAt {
    fn device(&self, path: &Path) -> io::Result<u64> {
        fs::symlink_metadata(path)?;
        Ok(u64::from(path.starts_with(&self.0)))
    }

    fn is_writable_dir(&self, _: &Path) -> io::Result<bool> {
        Ok(true)
    }
}

/// Makes one folder read-only to restore, like the Orange's root-owned bundled library.
struct ReadOnlyDir(PathBuf);

impl Filesystems for ReadOnlyDir {
    fn device(&self, path: &Path) -> io::Result<u64> {
        fs::symlink_metadata(path).map(|_| 0)
    }

    fn is_writable_dir(&self, path: &Path) -> io::Result<bool> {
        Ok(path != self.0)
    }
}

fn mounted_sd_card(fixture: &Fixture) -> MountedAt {
    let card = fixture.samples.join("sd-card");
    fs::create_dir_all(card.join("octessera/samples")).unwrap();
    fs::write(card.join("octessera/samples/kick.wav"), b"card sample").unwrap();
    MountedAt(card)
}

fn restore_on(
    fixture: &Fixture,
    filesystems: &dyn Filesystems,
    session: &str,
    media_included: bool,
) -> Result<(), String> {
    super::super::restore(
        filesystems,
        &fixture.store,
        &fixture.samples,
        &fixture.recordings,
        &fixture.screen_recordings,
        session,
        fixture.staged(media_included),
    )
}

#[test]
fn restore_keeps_a_mounted_sd_card_in_place_and_untouched() {
    for media_included in [false, true] {
        let session = format!("sd-card-{media_included}");
        let fixture = Fixture::new(&session);
        let card = mounted_sd_card(&fixture);

        restore_on(&fixture, &card, &session, media_included).unwrap();

        assert_eq!(
            fs::read(card.0.join("octessera/samples/kick.wav")).unwrap(),
            b"card sample"
        );
        assert!(!fixture
            .staging
            .join(format!(".octessera-samples-old-{session}"))
            .exists());
        assert!(!fixture
            .staging
            .join(format!(".octessera-samples-new-{session}"))
            .exists());
    }
}

#[test]
fn restore_leaves_a_read_only_library_folder_in_place() {
    for media_included in [false, true] {
        let session = format!("read-only-library-{media_included}");
        let fixture = Fixture::new(&session);
        let library = fixture.samples.join("Drum");
        fs::create_dir_all(library.join("kick")).unwrap();
        fs::write(library.join("kick/909.wav"), b"bundled kick").unwrap();

        restore_on(
            &fixture,
            &ReadOnlyDir(library.clone()),
            &session,
            media_included,
        )
        .unwrap();

        assert_eq!(
            fs::read(library.join("kick/909.wav")).unwrap(),
            b"bundled kick"
        );
        assert!(!fixture
            .staging
            .join(format!(".octessera-samples-old-{session}"))
            .exists());
    }
}

#[test]
fn rollback_after_a_full_swap_keeps_a_mounted_sd_card_in_place() {
    let (root, trees) = transaction_fixture("sd-card-rollback");
    let card = trees[1].0.join("sd-card");
    fs::create_dir_all(&card).unwrap();
    fs::write(card.join("kick.wav"), b"card sample").unwrap();
    let blocked = root.join("blocked");
    let blocked_new = root.join("blocked-new");
    fs::create_dir_all(&blocked_new).unwrap();
    fs::write(blocked_new.join("state"), b"new blocked").unwrap();
    let mut refs = tree_refs(&trees);
    let blocked_old = root.join("blocked-old");
    refs.push((&blocked, &blocked_new, &blocked_old));
    let mut faults = FaultInjection {
        move_in_failure_at: Some((2, 0)),
        ..FaultInjection::default()
    };

    assert!(
        replace_trees_with_faults(&MountedAt(card.clone()), &refs, &trees[1].0, &mut faults)
            .is_err()
    );

    assert_eq!(fs::read(trees[1].0.join("state")).unwrap(), b"live samples");
    assert_eq!(fs::read(card.join("kick.wav")).unwrap(), b"card sample");
    assert_eq!(fs::read(trees[1].1.join("state")).unwrap(), b"new samples");
    assert!(!trees[1].2.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn recovery_tree_removal_never_descends_into_another_filesystem() {
    let root = temp_root("removal-boundary");
    let old = root.join("old");
    fs::create_dir_all(old.join("sd-card")).unwrap();
    fs::write(old.join("sd-card/kick.wav"), b"card sample").unwrap();
    fs::write(old.join("rootfs.wav"), b"rootfs sample").unwrap();

    assert!(remove_tree_within(&MountedAt(old.join("sd-card")), &old, &old).is_err());

    assert_eq!(
        fs::read(old.join("sd-card/kick.wav")).unwrap(),
        b"card sample"
    );
    let _ = fs::remove_dir_all(root);
}

/// Records, whenever restore asks about a folder, whether someone else could take the SD lock.
#[cfg(unix)]
struct WatchesSdLock {
    lock: PathBuf,
    contended: std::cell::Cell<usize>,
    free: std::cell::Cell<usize>,
}

#[cfg(unix)]
impl Filesystems for WatchesSdLock {
    fn device(&self, path: &Path) -> io::Result<u64> {
        fs::symlink_metadata(path).map(|_| 0)
    }

    fn is_writable_dir(&self, _: &Path) -> io::Result<bool> {
        use std::os::fd::AsRawFd;
        let probe = fs::File::open(&self.lock)?;
        if unsafe { libc::flock(probe.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            self.free.set(self.free.get() + 1);
        } else {
            self.contended.set(self.contended.get() + 1);
        }
        Ok(true)
    }

    fn sd_card_lock_path(&self) -> &Path {
        &self.lock
    }
}

#[cfg(unix)]
#[test]
fn restore_holds_the_sd_card_lock_while_it_builds_and_swaps() {
    let fixture = Fixture::new("sd-lock");
    fs::create_dir_all(fixture.samples.join("My Kit")).unwrap();
    let lock = fixture.root.join("sd-card.lock");
    fs::write(&lock, b"").unwrap();
    let watcher = WatchesSdLock {
        lock: lock.clone(),
        contended: std::cell::Cell::new(0),
        free: std::cell::Cell::new(0),
    };

    restore_on(&fixture, &watcher, "sd-lock", true).unwrap();

    assert!(watcher.contended.get() > 0);
    assert_eq!(watcher.free.get(), 0);
    use std::os::fd::AsRawFd;
    let after = fs::File::open(&lock).unwrap();
    assert_eq!(
        unsafe { libc::flock(after.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
}
