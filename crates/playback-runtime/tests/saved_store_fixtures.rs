use serde_json::Value;
use std::fs;
use std::path::Path;

const PI_DEFAULT: &str = include_str!("../../../config/generated/pi/default.json");

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

// v0.8.14 keeps only its System: its patch predates the saved-patch contract, so no 0.8.14
// patch loads. Every later version keeps both files.
#[test]
fn every_released_store_loads() {
    let factory =
        playback_runtime::split_system_patch_documents(&serde_json::from_str(PI_DEFAULT).unwrap())
            .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/saved_stores");
    let mut versions = 0;
    for version in fs::read_dir(&root).unwrap() {
        let version = version.unwrap().path();
        let system = read_json(&version.join("system.json"));
        let patch_path = version.join("default.patch.json");
        let patch = if version.ends_with("v0.8.14") {
            factory.patch.clone()
        } else {
            read_json(&patch_path)
        };
        let full = playback_runtime::compose_local_system_patch_documents(&system, &patch)
            .unwrap_or_else(|error| panic!("{}: {error}", version.display()));
        playback_runtime::split_local_system_patch_documents(&full)
            .unwrap_or_else(|error| panic!("{}: {error}", version.display()));
        versions += 1;
    }
    assert!(versions > 0, "no saved stores under {}", root.display());
}
