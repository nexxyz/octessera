use std::fs;
use std::path::Path;

const FORBIDDEN_TERMS: &[&str] = &[
    concat!("tau", "ri"),
    concat!("src-", "tau", "ri"),
    concat!("Node", "Runner", "Process"),
    concat!("process", "_runner"),
    concat!("rp", "pal"),
    concat!("spi", "dev"),
    concat!("i2c", "dev"),
    concat!("Neo", "Key"),
    concat!("Neo", "Trellis"),
    concat!("neo", "key"),
    concat!("neo", "trellis"),
    "gpio",
    concat!("simulator", "Frame"),
    concat!("Simulator", "Frame"),
];

#[test]
fn core_stays_free_of_host_adapter_terms() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_no_forbidden_terms(&crate_dir.join("Cargo.toml"));
    scan_dir(&crate_dir.join("src"));
}

fn scan_dir(path: &Path) {
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan_dir(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            assert_no_forbidden_terms(&path);
        }
    }
}

fn assert_no_forbidden_terms(path: &Path) {
    let content = fs::read_to_string(path).unwrap();
    for term in FORBIDDEN_TERMS {
        assert!(
            !content.contains(term),
            "{} contains forbidden host adapter term `{}`",
            path.display(),
            term
        );
    }
}
