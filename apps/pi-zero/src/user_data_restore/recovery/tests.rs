use super::*;
use std::collections::BTreeMap;

const SESSION: &str = "s1";
const LABELS: [&str; 4] = ["store", "samples", "recordings", "screen-recordings"];

struct Boot {
    root: PathBuf,
}

impl Boot {
    fn new(name: &str) -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "octessera-restore-recovery-{name}-{}-{nonce}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join(STAGING_DIR_NAME)).unwrap();
        Self { root }
    }

    fn live(&self, label: &str) -> PathBuf {
        self.root.join(label)
    }

    fn staged(&self, label: &str, role: &str) -> PathBuf {
        let name = if matches!(label, "store" | "samples") {
            format!(".octessera-{label}-{role}-{SESSION}")
        } else {
            format!(".octessera-{role}-{label}-{SESSION}")
        };
        self.root.join(STAGING_DIR_NAME).join(name)
    }

    fn new_dir(&self, label: &str) -> PathBuf {
        self.staged(label, "new")
    }

    fn old_dir(&self, label: &str) -> PathBuf {
        self.staged(label, "old")
    }

    fn put_archive(&self) {
        fs::write(
            self.root
                .join(STAGING_DIR_NAME)
                .join(format!("octessera-pre-restore-{SESSION}.oct")),
            b"archive",
        )
        .unwrap();
    }

    fn put(&self, dir: &Path, files: &[(&str, &str)]) {
        fs::create_dir_all(dir).unwrap();
        for (name, text) in files {
            fs::write(dir.join(name), text).unwrap();
        }
    }

    fn untouched(&self, label: &str) {
        self.put(&self.live(label), &[("a", "old-a"), ("b", "old-b")]);
        self.put(&self.new_dir(label), &[("a", "new-a"), ("c", "new-c")]);
    }

    fn untouched_all(&self) {
        LABELS.iter().for_each(|label| self.untouched(label));
    }

    fn committed(&self, label: &str) {
        self.put(&self.live(label), &[("a", "new-a"), ("c", "new-c")]);
        self.put(&self.old_dir(label), &[("a", "old-a"), ("b", "old-b")]);
    }

    fn move_out(&self, label: &str, names: &[&str]) {
        for name in names {
            fs::rename(self.live(label).join(name), self.old_dir(label).join(name)).unwrap();
        }
    }

    fn sweep(&self) {
        finish_interrupted_restore(
            &LiveFilesystems,
            &self.live("store"),
            &self.live("samples"),
            &self.live("recordings"),
            &self.live("screen-recordings"),
        );
    }

    fn contents(&self, dir: &Path) -> BTreeMap<String, String> {
        fs::read_dir(dir)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                (
                    entry.file_name().into_string().unwrap(),
                    fs::read_to_string(entry.path()).unwrap(),
                )
            })
            .collect()
    }

    fn assert_all_restored(&self) {
        for label in LABELS {
            assert_eq!(
                self.contents(&self.live(label)),
                BTreeMap::from([
                    ("a".to_string(), "new-a".to_string()),
                    ("c".to_string(), "new-c".to_string())
                ]),
                "{label}"
            );
            assert!(!self.new_dir(label).exists(), "{label} new");
            assert!(!self.old_dir(label).exists(), "{label} old");
        }
    }
}

impl Drop for Boot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn begun(name: &str) -> Boot {
    let boot = Boot::new(name);
    boot.untouched_all();
    boot.put(&boot.old_dir("store"), &[]);
    boot.put_archive();
    boot
}

#[test]
fn build_phase_only_session_loses_its_new_folders_and_keeps_live() {
    let boot = Boot::new("build-phase");
    boot.untouched_all();
    boot.sweep();
    for label in LABELS {
        assert!(!boot.new_dir(label).exists());
        assert_eq!(boot.contents(&boot.live(label))["a"], "old-a");
        assert_eq!(boot.contents(&boot.live(label)).len(), 2);
    }
}

#[test]
fn swap_begun_with_nothing_moved_out_completes() {
    let boot = begun("empty-old");
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn swap_stopped_part_way_through_moving_out_completes() {
    let boot = begun("moving-out");
    boot.move_out("store", &["b"]);
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn swap_stopped_with_live_emptied_completes() {
    let boot = begun("live-emptied");
    boot.move_out("store", &["a", "b"]);
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn swap_stopped_part_way_through_moving_in_keeps_the_moved_in_entries() {
    let boot = begun("moving-in");
    boot.move_out("store", &["a", "b"]);
    fs::rename(
        boot.new_dir("store").join("c"),
        boot.live("store").join("c"),
    )
    .unwrap();
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn swap_stopped_with_everything_moved_in_but_the_new_folder_left_completes() {
    let boot = Boot::new("new-left");
    boot.committed("store");
    fs::create_dir(boot.new_dir("store")).unwrap();
    for label in &LABELS[1..] {
        boot.untouched(label);
    }
    boot.put_archive();
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn committed_trees_lose_their_old_folders_while_later_trees_roll_forward() {
    let boot = Boot::new("multi-tree");
    boot.committed("store");
    boot.committed("samples");
    boot.untouched("recordings");
    boot.untouched("screen-recordings");
    boot.put_archive();
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn a_missing_live_folder_is_created_when_the_swap_had_begun() {
    let boot = Boot::new("live-missing");
    boot.committed("store");
    boot.committed("samples");
    for label in ["recordings", "screen-recordings"] {
        boot.put(&boot.new_dir(label), &[("a", "new-a"), ("c", "new-c")]);
    }
    boot.put_archive();
    boot.sweep();
    boot.assert_all_restored();
}

#[test]
fn old_folders_stay_when_the_pre_restore_archive_is_missing() {
    let boot = Boot::new("no-archive");
    boot.committed("store");
    for label in &LABELS[1..] {
        boot.untouched(label);
    }
    boot.sweep();
    assert_eq!(boot.contents(&boot.old_dir("store")).len(), 2);
    assert_eq!(boot.contents(&boot.live("store"))["a"], "new-a");
    assert_eq!(boot.contents(&boot.live("samples"))["a"], "new-a");
}

#[test]
fn an_error_stops_the_session_and_leaves_the_failing_tree_as_it_was() {
    let boot = Boot::new("error");
    boot.put(&boot.live("store").join("d"), &[("x", "live-x")]);
    boot.put(&boot.old_dir("store").join("d"), &[("x", "old-x")]);
    boot.put(&boot.new_dir("store").join("d"), &[("y", "new-y")]);
    for label in &LABELS[1..] {
        boot.untouched(label);
    }
    boot.put_archive();
    boot.sweep();
    let read = |path: PathBuf| fs::read_to_string(path).unwrap();
    assert_eq!(read(boot.live("store").join("d").join("x")), "live-x");
    assert_eq!(read(boot.old_dir("store").join("d").join("x")), "old-x");
    assert!(boot.new_dir("store").join("d").join("y").exists());
    for label in &LABELS[1..] {
        assert_eq!(boot.contents(&boot.live(label))["a"], "old-a");
        assert!(boot.new_dir(label).exists());
    }
}
