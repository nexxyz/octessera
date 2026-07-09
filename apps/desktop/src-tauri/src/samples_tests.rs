use super::*;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{nonce}"));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn touch(path: &Path) {
    fs::write(path, b"x").expect("write file");
}

#[test]
fn sanitize_relative_dir_rejects_absolute_and_parent_traversal() {
    assert!(sanitize_relative_dir("../x").is_err());
    assert!(sanitize_relative_dir("a/../x").is_err());
    assert!(sanitize_relative_dir("/abs").is_err());
    assert!(sanitize_relative_dir(r"\abs").is_err());
}

#[test]
fn sanitize_relative_dir_normalizes_separator_and_dots() {
    assert_eq!(sanitize_relative_dir(r"a\b//c").expect("sanitize"), "a/b/c");
    assert_eq!(sanitize_relative_dir(" ./a//b/ ").expect("sanitize"), "a/b");
}

#[test]
fn resolve_sample_file_from_root_accepts_only_wav_inside_root() {
    let root = unique_temp_dir("octessera_samples_resolve");
    let sub = root.join("drums");
    fs::create_dir_all(&sub).expect("subdir");
    let wav = sub.join("kick.wav");
    let txt = sub.join("readme.txt");
    touch(&wav);
    touch(&txt);
    assert!(resolve_sample_file_from_roots(&root, &root, "drums/kick.wav").is_some());
    assert!(resolve_sample_file_from_roots(&root, &root, "drums/readme.txt").is_none());
    assert!(resolve_sample_file_from_roots(&root, &root, "drums").is_none());
    assert!(resolve_sample_file_from_roots(&root, &root, "../outside.wav").is_none());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn sample_list_from_root_sorts_dirs_first_and_filters_wav() {
    let root = unique_temp_dir("octessera_samples_list");
    let drums = root.join("Drums");
    fs::create_dir_all(&drums).expect("drums dir");
    touch(&root.join("b.wav"));
    touch(&root.join("A.WAV"));
    touch(&root.join("ignore.mp3"));
    let entries = sample_list_from_root(&root, "").expect("list");
    assert!(!entries.is_empty());
    assert!(entries[0].is_dir);
    assert_eq!(entries[0].name, "Drums");
    let file_names: Vec<String> = entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.name.clone())
        .collect();
    assert_eq!(file_names, vec!["A.WAV".to_string(), "b.wav".to_string()]);
    let _ = fs::remove_dir_all(&root);
}
