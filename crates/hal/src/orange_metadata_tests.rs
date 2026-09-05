use super::{
    hash_file, metadata_path, parse_metadata, print_runtime_benchmark_diagnostic_metadata,
    read_metadata_text, validate_executable_name, validate_executable_name_for, validate_metadata,
    validate_metadata_for, validate_runtime_benchmark_diagnostic_metadata,
    validate_runtime_candidate_metadata, BuildMetadata, CANONICAL_BINARY_NAME, MAX_METADATA_BYTES,
    RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128, RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_256,
    RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING,
    RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_128,
    RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_256, RUNTIME_CANDIDATE_BINARY_NAME,
    SEESAW_BINARY_NAME,
};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn valid_json_for(binary: &str, hash: &str) -> String {
    let source_commit = valid_source_commit();
    format!(
        "{{\"schema_version\":2,\"board_profile\":\"orange-pi-zero-2w\",\"artifact_kind\":\"diagnostic-only\",\"runtime_ready\":false,\"binary\":\"{binary}\",\"package\":\"octessera-hal\",\"arch\":\"aarch64-unknown-linux-gnu\",\"cargo_feature\":\"orange-pi-zero-2w\",\"profile\":\"pi-dev\",\"binary_sha256\":\"{hash}\",\"source_commit\":\"{source_commit}\"}}"
    )
}

fn valid_source_commit() -> String {
    "a".repeat(40)
}

fn valid_json(hash: &str) -> String {
    valid_json_for(CANONICAL_BINARY_NAME, hash)
}

fn valid_metadata_for(binary: &str, hash: &str) -> BuildMetadata {
    BuildMetadata {
        schema_version: 2,
        board_profile: "orange-pi-zero-2w".into(),
        artifact_kind: "diagnostic-only".into(),
        runtime_ready: false,
        binary: binary.into(),
        package: "octessera-hal".into(),
        arch: "aarch64-unknown-linux-gnu".into(),
        cargo_feature: "orange-pi-zero-2w".into(),
        profile: "release".into(),
        binary_sha256: hash.into(),
        source_commit: valid_source_commit(),
    }
}

fn valid_metadata(hash: &str) -> BuildMetadata {
    valid_metadata_for(CANONICAL_BINARY_NAME, hash)
}

fn valid_runtime_benchmark_metadata(cargo_feature: &str, hash: &str) -> BuildMetadata {
    let mut metadata = valid_metadata_for(RUNTIME_CANDIDATE_BINARY_NAME, hash);
    metadata.artifact_kind = "diagnostic-only".into();
    metadata.package = "octessera-pi".into();
    metadata.cargo_feature = cargo_feature.into();
    metadata
}

fn temporary_path() -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock is after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("octessera-orange-metadata-{stamp}"))
}

type MetadataMutation = (&'static str, fn(&mut BuildMetadata));

#[test]
fn metadata_parser_rejects_malformed_content_table() {
    let hash = "a".repeat(64);
    let metadata = parse_metadata(&valid_json(&hash)).expect("valid metadata");
    assert_eq!(metadata.binary, CANONICAL_BINARY_NAME);
    assert_eq!(metadata.source_commit, valid_source_commit());
    assert!(validate_metadata(&metadata, &hash).is_ok());
    let cases = [
        (
            "missing field",
            valid_json(&hash).replace("\"profile\":\"pi-dev\",", ""),
        ),
        (
            "extra field",
            valid_json(&hash).replace("}", ",\"extra\":1}"),
        ),
        (
            "duplicate field",
            valid_json(&hash).replace(
                "\"profile\":\"pi-dev\"",
                "\"profile\":\"pi-dev\",\"profile\":\"release\"",
            ),
        ),
        (
            "missing source commit",
            valid_json(&hash).replace(
                &format!(",\"source_commit\":\"{}\"", valid_source_commit()),
                "",
            ),
        ),
        ("malformed JSON", "{\"schema_version\":2".to_string()),
        (
            "malformed field type",
            valid_json(&hash).replace("\"runtime_ready\":false", "\"runtime_ready\":0"),
        ),
    ];
    for (label, input) in cases {
        assert!(
            parse_metadata(&input).is_err(),
            "{label} unexpectedly parsed"
        );
    }
}

#[test]
fn source_commit_is_required_and_canonical() {
    let hash = "b".repeat(64);
    let malformed = [
        String::new(),
        "a".repeat(39),
        "a".repeat(41),
        "A".repeat(40),
        "g".repeat(40),
    ];
    for source_commit in malformed {
        let mut metadata = valid_metadata(&hash);
        metadata.source_commit = source_commit;
        assert!(validate_metadata(&metadata, &hash).is_err());
    }
}

#[test]
fn metadata_identity_fields_are_rejected_independently() {
    let hash = "b".repeat(64);
    let mutations: [MetadataMutation; 10] = [
        ("schema_version", |metadata| metadata.schema_version = 1),
        ("board_profile", |metadata| {
            metadata.board_profile = "other".into()
        }),
        ("artifact_kind", |metadata| {
            metadata.artifact_kind = "runtime".into()
        }),
        ("runtime_ready", |metadata| metadata.runtime_ready = true),
        ("binary", |metadata| metadata.binary = "other".into()),
        ("package", |metadata| metadata.package = "other".into()),
        ("arch", |metadata| metadata.arch = "other".into()),
        ("cargo_feature", |metadata| {
            metadata.cargo_feature = "other".into()
        }),
        ("profile", |metadata| metadata.profile = "other".into()),
        ("binary_sha256", |metadata| {
            metadata.binary_sha256 = "c".repeat(64)
        }),
    ];
    for (field, mutate) in mutations {
        let mut metadata = valid_metadata(&hash);
        mutate(&mut metadata);
        assert!(
            validate_metadata(&metadata, &hash).is_err(),
            "identity field {field} was accepted"
        );
    }
}

#[test]
fn metadata_and_executable_hashes_reject_malformed_values() {
    let valid_hash = "b".repeat(64);
    for hash in [
        "",
        &"b".repeat(63),
        &"b".repeat(65),
        &"B".repeat(64),
        &"g".repeat(64),
    ] {
        let mut metadata = valid_metadata(&valid_hash);
        metadata.binary_sha256 = hash.to_string();
        assert!(validate_metadata(&metadata, &valid_hash).is_err());
        assert!(validate_metadata(&valid_metadata(&valid_hash), hash).is_err());
    }
}

#[test]
fn hash_is_streamed_from_a_file_and_sidecar_is_adjacent() {
    let directory = temporary_path();
    fs::create_dir_all(&directory).expect("temporary directory");
    let binary = directory.join(CANONICAL_BINARY_NAME);
    fs::write(&binary, b"orange diagnostic ELF placeholder").expect("temporary binary");
    let hash = hash_file(&binary).expect("binary hash");
    assert_eq!(hash.len(), 64);
    assert_eq!(
        metadata_path(Path::new("/tmp/orange-oled-smoke")),
        Path::new("/tmp/orange-oled-smoke.metadata.json")
    );
    fs::remove_dir_all(directory).expect("temporary directory cleanup");
}

#[test]
fn sidecar_input_is_bounded_before_parsing() {
    let directory = temporary_path();
    fs::create_dir_all(&directory).expect("temporary directory");
    let metadata = directory.join("orange-oled-smoke.metadata.json");
    fs::write(&metadata, vec![b' '; MAX_METADATA_BYTES + 1]).expect("oversized metadata");
    assert!(read_metadata_text(&metadata).is_err());
    fs::remove_dir_all(directory).expect("temporary directory cleanup");
}

#[test]
fn binary_tampering_breaks_hash_bound_validation() {
    let directory = temporary_path();
    fs::create_dir_all(&directory).expect("temporary directory");
    let binary = directory.join(CANONICAL_BINARY_NAME);
    fs::write(&binary, b"original diagnostic ELF placeholder").expect("temporary binary");
    let metadata_hash = hash_file(&binary).expect("original hash");
    let metadata = valid_metadata(&metadata_hash);
    fs::write(&binary, b"tampered diagnostic ELF placeholder").expect("tampered binary");
    let tampered_hash = hash_file(&binary).expect("tampered hash");
    assert_ne!(metadata_hash, tampered_hash);
    assert!(validate_metadata(&metadata, &tampered_hash).is_err());
    fs::remove_dir_all(directory).expect("temporary directory cleanup");
}

#[test]
fn executable_name_must_be_canonical() {
    assert!(validate_executable_name(Path::new(CANONICAL_BINARY_NAME)).is_ok());
    assert!(validate_executable_name(Path::new("octessera-pi")).is_err());
}

#[test]
fn seesaw_metadata_uses_the_same_hash_bound_identity_contract() {
    let hash = "d".repeat(64);
    let metadata = valid_metadata_for(SEESAW_BINARY_NAME, &hash);
    assert!(validate_metadata_for(&metadata, &hash, SEESAW_BINARY_NAME).is_ok());
    assert!(
        validate_executable_name_for(Path::new(SEESAW_BINARY_NAME), SEESAW_BINARY_NAME).is_ok()
    );
    assert!(validate_metadata(&metadata, &hash).is_err());
    assert!(parse_metadata(&valid_json_for(SEESAW_BINARY_NAME, &hash)).is_ok());
}

#[test]
fn runtime_candidate_metadata_is_hash_bound_but_not_runtime_ready() {
    let hash = "e".repeat(64);
    let mut metadata = valid_metadata_for(RUNTIME_CANDIDATE_BINARY_NAME, &hash);
    metadata.artifact_kind = "runtime-candidate".into();
    metadata.package = "octessera-pi".into();
    metadata.cargo_feature = "hardware-orange-pi-zero-2w".into();
    assert!(validate_runtime_candidate_metadata(&metadata, &hash).is_ok());
    metadata.cargo_feature = "orange-pi-zero-2w".into();
    assert!(validate_runtime_candidate_metadata(&metadata, &hash).is_err());
    metadata.cargo_feature = "hardware-orange-pi-zero-2w".into();
    metadata.runtime_ready = true;
    assert!(validate_runtime_candidate_metadata(&metadata, &hash).is_err());
    metadata.runtime_ready = false;
    assert!(validate_runtime_candidate_metadata(&metadata, &"f".repeat(64)).is_err());
}

#[test]
fn runtime_benchmark_diagnostic_accepts_only_exact_supported_features() {
    let hash = "e".repeat(64);
    for cargo_feature in [
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128,
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_256,
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING,
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_128,
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_ROUTING_256,
    ] {
        let metadata = valid_runtime_benchmark_metadata(cargo_feature, &hash);
        assert!(
            validate_runtime_benchmark_diagnostic_metadata(&metadata, &hash, cargo_feature).is_ok()
        );
    }
}

#[test]
fn runtime_benchmark_diagnostic_print_rejects_an_unapproved_feature() {
    let error = print_runtime_benchmark_diagnostic_metadata(
        "hardware-orange-pi-zero-2w benchmark-voice-pools-512",
    )
    .unwrap_err();
    assert_eq!(
        error,
        "runtime benchmark diagnostic metadata requires an exact routing or 128/256 voice-pool cargo feature"
    );
}

#[test]
fn runtime_benchmark_diagnostic_rejects_wrong_stage_kind_package_feature_and_hash() {
    let hash = "e".repeat(64);
    let feature = RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128;

    let mut metadata = valid_runtime_benchmark_metadata(feature, &hash);
    metadata.runtime_ready = true;
    assert!(validate_runtime_benchmark_diagnostic_metadata(&metadata, &hash, feature).is_err());

    let mut metadata = valid_runtime_benchmark_metadata(feature, &hash);
    metadata.artifact_kind = "runtime-candidate".into();
    assert!(validate_runtime_benchmark_diagnostic_metadata(&metadata, &hash, feature).is_err());

    let mut metadata = valid_runtime_benchmark_metadata(feature, &hash);
    metadata.package = "octessera-hal".into();
    assert!(validate_runtime_benchmark_diagnostic_metadata(&metadata, &hash, feature).is_err());

    let mut metadata =
        valid_runtime_benchmark_metadata(RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_256, &hash);
    assert!(validate_runtime_benchmark_diagnostic_metadata(&metadata, &hash, feature).is_err());
    metadata.cargo_feature = "hardware-orange-pi-zero-2w benchmark-voice-pools-512".into();
    assert!(validate_runtime_benchmark_diagnostic_metadata(
        &metadata,
        &hash,
        metadata.cargo_feature.as_str()
    )
    .is_err());

    let metadata = valid_runtime_benchmark_metadata(feature, &hash);
    assert!(
        validate_runtime_benchmark_diagnostic_metadata(&metadata, &"f".repeat(64), feature)
            .is_err()
    );
}

#[test]
fn metadata_mismatch_errors_name_the_contract() {
    let hash = "a".repeat(64);
    let mut diagnostic = valid_metadata(&hash);
    diagnostic.package = "wrong-package".into();
    let diagnostic_error = validate_metadata(&diagnostic, &hash).unwrap_err();
    assert_eq!(
        diagnostic_error,
        "metadata identity fields do not match the Orange diagnostic-only contract"
    );

    let mut candidate = valid_metadata_for(RUNTIME_CANDIDATE_BINARY_NAME, &hash);
    candidate.artifact_kind = "runtime-candidate".into();
    candidate.package = "octessera-pi".into();
    candidate.package.push_str("-tampered");
    let candidate_error = validate_runtime_candidate_metadata(&candidate, &hash).unwrap_err();
    assert_eq!(
        candidate_error,
        "metadata identity fields do not match the Orange runtime-candidate contract"
    );

    let mut benchmark =
        valid_runtime_benchmark_metadata(RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128, &hash);
    benchmark.package = "wrong-package".into();
    let benchmark_error = validate_runtime_benchmark_diagnostic_metadata(
        &benchmark,
        &hash,
        RUNTIME_BENCHMARK_DIAGNOSTIC_CARGO_FEATURE_128,
    )
    .unwrap_err();
    assert_eq!(
        benchmark_error,
        "metadata identity fields do not match the Orange runtime benchmark diagnostic contract"
    );
}
