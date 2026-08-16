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

fn symlink_dir(link: &Path, target: &Path) -> std::io::Result<()> {
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
fn sanitize_relative_dir_rejects_absolute_and_parent_traversal() {
    assert!(sanitize_relative_dir("../x").is_err());
    assert!(sanitize_relative_dir("a/../x").is_err());
    assert!(sanitize_relative_dir("/abs").is_err());
    assert!(sanitize_relative_dir(r"\abs").is_err());
    assert!(sanitize_relative_dir(r"C:\samples").is_err());
    assert!(canonical_sample_relative_path("sd-card/kick.wav").is_err());
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
fn resolve_sample_file_handles_canonical_prefix_spaces_and_separate_user_data() {
    let root = unique_temp_dir("octessera_samples_resource with spaces");
    let user_root = unique_temp_dir("octessera_user_samples with spaces");
    let bundled = root.join("Drum Kit");
    let user = user_root.join("User Kit");
    fs::create_dir_all(&bundled).expect("bundled dir");
    fs::create_dir_all(&user).expect("user dir");
    touch(&bundled.join("kick.wav"));
    touch(&user.join("custom.wav"));

    assert!(
        resolve_sample_file_from_roots(&root, &user_root, "samples/Drum Kit/kick.wav").is_some()
    );
    assert!(
        resolve_sample_file_from_roots(&root, &user_root, "userdata/User Kit/custom.wav").is_some()
    );
    assert!(resolve_sample_file_from_roots(
        &root,
        &user_root,
        "userdata/samples/User Kit/custom.wav"
    )
    .is_some());
    assert!(
        resolve_sample_file_from_roots(&root, &user_root, "samples/Drum Kit/missing.wav").is_none()
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(user_root);
}

#[test]
fn bundled_resolution_does_not_require_a_user_samples_root() {
    let root = unique_temp_dir("octessera_bundled_samples_only");
    let missing_user_root = root.join("user-data-that-must-not-be-created");
    fs::create_dir_all(root.join("Drum")).expect("drum dir");
    touch(&root.join("Drum").join("kick.wav"));

    assert!(
        resolve_sample_file_from_roots(&root, &missing_user_root, "samples/Drum/kick.wav")
            .is_some()
    );
    assert!(!missing_user_root.exists());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_bundled_sample_does_not_fall_back_to_user_samples() {
    let root = unique_temp_dir("octessera_missing_bundled_samples");
    let user_root = unique_temp_dir("octessera_fallback_user_samples");
    touch(&user_root.join("kick.wav"));

    assert!(
        resolve_sample_file_from_roots(&root.join("missing"), &user_root, "samples/kick.wav")
            .is_none()
    );

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(user_root);
}

#[test]
fn user_browser_entries_keep_the_userdata_identity() {
    let root = unique_temp_dir("octessera_bundled_samples");
    let user_root = unique_temp_dir("octessera_user_samples");
    fs::create_dir_all(root.join("Drum")).expect("drum dir");
    fs::create_dir_all(user_root.join("User Kit")).expect("user kit dir");
    touch(&root.join("Drum").join("kick.wav"));
    touch(&user_root.join("User Kit").join("custom.wav"));

    let entries = sample_list_from_roots(&root, &user_root, "userdata").expect("user list");
    assert_eq!(entries[0].path, "userdata/User Kit");
    let entries =
        sample_list_from_roots(&root, &user_root, "userdata/User Kit").expect("user kit list");
    assert_eq!(entries[0].path, "userdata/User Kit/custom.wav");

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(user_root);
}

#[test]
fn sample_resolution_rejects_symlinked_components() {
    let root = unique_temp_dir("octessera_samples_symlink");
    let outside = unique_temp_dir("octessera_samples_outside");
    touch(&outside.join("escape.wav"));
    let linked = root.join("linked");
    if let Err(error) = symlink_dir(&linked, &outside) {
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
        panic!("symlink creation unavailable: {error}");
    }

    assert!(resolve_sample_file_from_roots(&root, &root, "linked/escape.wav").is_none());
    assert!(sample_list_from_root(&root, "").is_err());

    let _ = fs::remove_dir_all(root);
    let _ = fs::remove_dir_all(outside);
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
    assert_eq!(entries[0].path, "samples/Drums");
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.name == "A.WAV")
            .expect("A.WAV")
            .path,
        "samples/A.WAV"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn desktop_resource_fixture_decodes_every_playable_default_sample() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../samples");
    let root = root.canonicalize().expect("desktop sample fixture root");
    let user_root = root.join("missing-user-samples");
    let browser_entries = sample_list_from_root(&root, "samples/Drum/kick")
        .expect("desktop resource browser fixture");
    assert!(browser_entries
        .iter()
        .any(|entry| { entry.name == "Kick2.wav" && entry.path == "samples/Drum/kick/Kick2.wav" }));
    let mut playable_wav_count = 0;
    let mut metadata_only_count = 0;

    for line in include_str!("../../../../samples/ATTRIBUTIONS.tsv")
        .lines()
        .skip(1)
    {
        let relative = line.split('\t').next().expect("inventory path");
        if relative.to_ascii_lowercase().ends_with(".wav") {
            playable_wav_count += 1;
            let canonical_id = format!("samples/{relative}");
            let resolved = resolve_sample_file_from_roots(&root, &user_root, &canonical_id)
                .expect("manifest WAV resolves from desktop resource root");
            let expected = root
                .join(relative)
                .canonicalize()
                .expect("manifest WAV path");
            assert_eq!(PathBuf::from(&resolved), expected);
            let buffer = rodio_engine_source::decode_sample_file(&resolved)
                .expect("manifest WAV decodes with production decoder");
            assert!(buffer.channels > 0);
            assert!(buffer.sample_rate > 0);
            assert!(!buffer.samples.is_empty());
            assert_eq!(buffer.samples.len() % usize::from(buffer.channels), 0);
            assert!(buffer.samples.iter().all(|sample| sample.is_finite()));
            assert!(expected.starts_with(&root));
        } else {
            metadata_only_count += 1;
        }
    }

    assert_eq!(playable_wav_count, 318);
    assert_eq!(metadata_only_count, 2);
}
