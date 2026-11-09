use super::super::filesystem::PlacementError;
use super::*;

const LIVE: [&str; 4] = ["a", "b", "c", "d"];
const NEW: [&str; 3] = ["x", "y", "z"];

/// Two live folders with several entries each, and their replacements.
fn several_entries(name: &str) -> (PathBuf, Vec<(PathBuf, PathBuf, PathBuf)>) {
    let root = temp_root(name);
    let mut trees = Vec::new();
    for tree in ["store", "samples"] {
        let current = root.join(tree);
        let replacement = root.join(format!("{tree}-new"));
        fs::create_dir_all(&current).unwrap();
        fs::create_dir_all(&replacement).unwrap();
        for entry in LIVE {
            fs::write(current.join(entry), format!("live {tree} {entry}")).unwrap();
        }
        for entry in NEW {
            fs::write(replacement.join(entry), format!("new {tree} {entry}")).unwrap();
        }
        trees.push((current, replacement, root.join(format!("{tree}-old"))));
    }
    (root, trees)
}

fn assert_rolled_back(trees: &[(PathBuf, PathBuf, PathBuf)]) {
    for ((current, replacement, old), tree) in trees.iter().zip(["store", "samples"]) {
        let mut live = entry_names(current);
        live.sort();
        assert_eq!(live, LIVE, "{tree}");
        for entry in LIVE {
            assert_eq!(
                fs::read_to_string(current.join(entry)).unwrap(),
                format!("live {tree} {entry}")
            );
        }
        let mut staged = entry_names(replacement);
        staged.sort();
        assert_eq!(staged, NEW, "{tree}");
        assert!(!old.exists(), "{tree}");
    }
}

fn entry_names(path: &Path) -> Vec<String> {
    fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect()
}

fn fails_with(faults: FaultInjection, name: &str, phase: &str) {
    let (root, trees) = several_entries(name);
    let refs = tree_refs(&trees);
    let mut faults = faults;

    let error =
        replace_trees_with_faults(&LiveFilesystems, &refs, &trees[1].0, &mut faults).unwrap_err();

    assert!(error.starts_with(phase), "{error}");
    assert!(!error.contains("rollback failed"), "{error}");
    assert_rolled_back(&trees);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn failing_the_nth_move_out_rolls_back_every_entry() {
    for (tree, nth) in [(0, 2), (1, 3), (1, 0)] {
        fails_with(
            FaultInjection {
                move_out_failure_at: Some((tree, nth)),
                ..FaultInjection::default()
            },
            &format!("move-out-{tree}-{nth}"),
            "restore could not move live tree",
        );
    }
}

#[test]
fn failing_the_nth_move_in_rolls_back_every_entry() {
    for (tree, nth) in [(0, 1), (1, 2), (1, 0)] {
        fails_with(
            FaultInjection {
                move_in_failure_at: Some((tree, nth)),
                ..FaultInjection::default()
            },
            &format!("move-in-{tree}-{nth}"),
            "restore replacement swap failed",
        );
    }
}

#[test]
fn rollback_removes_a_live_folder_the_restore_created() {
    let (root, trees) = several_entries("created-live-folder");
    fs::remove_dir_all(&trees[0].0).unwrap();
    let refs = tree_refs(&trees);
    let mut faults = FaultInjection {
        move_in_failure_at: Some((1, 1)),
        ..FaultInjection::default()
    };

    replace_trees_with_faults(&LiveFilesystems, &refs, &trees[1].0, &mut faults).unwrap_err();

    assert!(!trees[0].0.exists());
    assert!(!trees[0].2.exists());
    let mut staged = entry_names(&trees[0].1);
    staged.sort();
    assert_eq!(staged, NEW);
    let _ = fs::remove_dir_all(root);
}

/// Folders restore may not write, by path.
struct ReadOnlyDirs(Vec<PathBuf>);

impl Filesystems for ReadOnlyDirs {
    fn device(&self, path: &Path) -> io::Result<u64> {
        fs::symlink_metadata(path).map(|_| 0)
    }

    fn is_writable_dir(&self, path: &Path) -> io::Result<bool> {
        Ok(!self.0.iter().any(|dir| dir == path))
    }
}

#[test]
fn only_bundled_library_folders_in_samples_may_stay_unwritable() {
    let root = temp_root("placement");
    let samples = root.join("samples");
    let recordings = root.join("recordings");
    for dir in [
        samples.join("Drum"),
        samples.join("My Kit"),
        samples.join("Drum/kick"),
        recordings.join("Drum"),
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    let filesystems = ReadOnlyDirs(vec![
        samples.join("Drum"),
        samples.join("My Kit"),
        samples.join("Drum/kick"),
        recordings.join("Drum"),
    ]);
    let unmovable = |root: &Path, path: PathBuf, holds_library| {
        matches!(
            filesystems.stays_in_place(root, &path, holds_library),
            Err(PlacementError::Unmovable(blocked)) if blocked == path
        )
    };

    assert!(filesystems
        .stays_in_place(&samples, &samples.join("Drum"), true)
        .unwrap());
    assert!(unmovable(&samples, samples.join("My Kit"), true));
    assert!(unmovable(&samples, samples.join("Drum/kick"), true));
    assert!(unmovable(&recordings, recordings.join("Drum"), false));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn an_unwritable_folder_fails_the_restore_and_keeps_live_data() {
    for media_included in [false, true] {
        let session = format!("unwritable-{media_included}");
        let fixture = Fixture::new(&session);
        let kit = fixture.samples.join("My Kit");
        fs::create_dir_all(&kit).unwrap();
        fs::write(kit.join("snare.wav"), b"my snare").unwrap();

        let error = super::super::restore(
            &ReadOnlyDirs(vec![kit.clone()]),
            &fixture.store,
            &fixture.samples,
            &fixture.recordings,
            &fixture.screen_recordings,
            &session,
            fixture.staged(media_included),
        )
        .unwrap_err();

        assert!(error.contains("restore cannot move"), "{error}");
        assert!(error.contains("My Kit"), "{error}");
        fixture.assert_live_data();
        assert_eq!(fs::read(kit.join("snare.wav")).unwrap(), b"my snare");
    }
}

#[cfg(unix)]
#[test]
fn writability_errors_other_than_permission_are_returned() {
    let root = temp_root("writability");

    assert!(LiveFilesystems.is_writable_dir(&root).unwrap());
    assert_eq!(
        LiveFilesystems
            .is_writable_dir(&root.join("missing"))
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    let _ = fs::remove_dir_all(root);
}
