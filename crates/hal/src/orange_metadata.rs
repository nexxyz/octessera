use serde::de::{self, MapAccess, Visitor};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::env;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub const CANONICAL_BINARY_NAME: &str = "orange-oled-smoke";
pub const SEESAW_BINARY_NAME: &str = "orange-seesaw-smoke";
pub const RUNTIME_CANDIDATE_BINARY_NAME: &str = "octessera-pi";
pub const METADATA_SUFFIX: &str = ".metadata.json";
pub const METADATA_SCHEMA_VERSION: u64 = 2;
pub const MAX_METADATA_BYTES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuildMetadata {
    pub schema_version: u64,
    pub board_profile: String,
    pub artifact_kind: String,
    pub runtime_ready: bool,
    pub binary: String,
    pub package: String,
    pub arch: String,
    pub cargo_feature: String,
    pub profile: String,
    pub binary_sha256: String,
}

impl<'de> Deserialize<'de> for BuildMetadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BuildMetadataVisitor;

        impl<'de> Visitor<'de> for BuildMetadataVisitor {
            type Value = BuildMetadata;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an exact Orange diagnostic metadata object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<BuildMetadata, A::Error>
            where
                A: MapAccess<'de>,
            {
                const FIELDS: &[&str] = &[
                    "schema_version",
                    "board_profile",
                    "artifact_kind",
                    "runtime_ready",
                    "binary",
                    "package",
                    "arch",
                    "cargo_feature",
                    "profile",
                    "binary_sha256",
                ];
                let mut schema_version = None;
                let mut board_profile = None;
                let mut artifact_kind = None;
                let mut runtime_ready = None;
                let mut binary = None;
                let mut package = None;
                let mut arch = None;
                let mut cargo_feature = None;
                let mut profile = None;
                let mut binary_sha256 = None;

                macro_rules! next_field {
                    ($slot:ident, $name:literal) => {{
                        if $slot.is_some() {
                            return Err(de::Error::duplicate_field($name));
                        }
                        $slot = Some(map.next_value()?);
                    }};
                }

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "schema_version" => next_field!(schema_version, "schema_version"),
                        "board_profile" => next_field!(board_profile, "board_profile"),
                        "artifact_kind" => next_field!(artifact_kind, "artifact_kind"),
                        "runtime_ready" => next_field!(runtime_ready, "runtime_ready"),
                        "binary" => next_field!(binary, "binary"),
                        "package" => next_field!(package, "package"),
                        "arch" => next_field!(arch, "arch"),
                        "cargo_feature" => next_field!(cargo_feature, "cargo_feature"),
                        "profile" => next_field!(profile, "profile"),
                        "binary_sha256" => next_field!(binary_sha256, "binary_sha256"),
                        _ => return Err(de::Error::unknown_field(&key, FIELDS)),
                    }
                }

                Ok(BuildMetadata {
                    schema_version: schema_version
                        .ok_or_else(|| de::Error::missing_field("schema_version"))?,
                    board_profile: board_profile
                        .ok_or_else(|| de::Error::missing_field("board_profile"))?,
                    artifact_kind: artifact_kind
                        .ok_or_else(|| de::Error::missing_field("artifact_kind"))?,
                    runtime_ready: runtime_ready
                        .ok_or_else(|| de::Error::missing_field("runtime_ready"))?,
                    binary: binary.ok_or_else(|| de::Error::missing_field("binary"))?,
                    package: package.ok_or_else(|| de::Error::missing_field("package"))?,
                    arch: arch.ok_or_else(|| de::Error::missing_field("arch"))?,
                    cargo_feature: cargo_feature
                        .ok_or_else(|| de::Error::missing_field("cargo_feature"))?,
                    profile: profile.ok_or_else(|| de::Error::missing_field("profile"))?,
                    binary_sha256: binary_sha256
                        .ok_or_else(|| de::Error::missing_field("binary_sha256"))?,
                })
            }
        }

        deserializer.deserialize_map(BuildMetadataVisitor)
    }
}

pub fn print_build_metadata() -> Result<(), String> {
    print_build_metadata_for(CANONICAL_BINARY_NAME)
}

pub fn print_runtime_candidate_metadata() -> Result<(), String> {
    print_metadata_for(
        RUNTIME_CANDIDATE_BINARY_NAME,
        "runtime-candidate",
        "octessera-pi",
        "hardware-orange-pi-zero-2w",
    )
}

pub fn print_build_metadata_for(expected_binary: &str) -> Result<(), String> {
    print_metadata_for(
        expected_binary,
        "diagnostic-only",
        "octessera-hal",
        "orange-pi-zero-2w",
    )
}

fn print_metadata_for(
    expected_binary: &str,
    artifact_kind: &str,
    package: &str,
    cargo_feature: &str,
) -> Result<(), String> {
    let executable_location = env::current_exe()
        .map_err(|error| format!("cannot locate executable for metadata sidecar: {error}"))?;
    validate_executable_name_for(&executable_location, expected_binary)?;
    let metadata_path = metadata_path(&executable_location);
    let metadata_text = read_metadata_text(&metadata_path)?;
    let metadata = parse_metadata(&metadata_text)?;
    let executable_hash = hash_running_executable()?;
    validate_metadata_contract(
        &metadata,
        &executable_hash,
        expected_binary,
        artifact_kind,
        package,
        cargo_feature,
        if artifact_kind == "runtime-candidate" {
            "runtime-candidate"
        } else {
            "diagnostic-only"
        },
    )?;
    println!("{}", metadata_text.trim_end());
    Ok(())
}

pub fn metadata_path(binary: &Path) -> PathBuf {
    let mut path = binary.as_os_str().to_os_string();
    path.push(METADATA_SUFFIX);
    PathBuf::from(path)
}

pub fn parse_metadata(input: &str) -> Result<BuildMetadata, String> {
    serde_json::from_str(input).map_err(|error| format!("metadata JSON is invalid: {error}"))
}

fn read_metadata_text(path: &Path) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("cannot read adjacent metadata {}: {error}", path.display()))?;
    let mut bytes = Vec::with_capacity(MAX_METADATA_BYTES + 1);
    file.by_ref()
        .take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read adjacent metadata {}: {error}", path.display()))?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(format!(
            "adjacent metadata exceeds the {MAX_METADATA_BYTES}-byte limit: {}",
            path.display()
        ));
    }
    String::from_utf8(bytes).map_err(|error| {
        format!(
            "adjacent metadata is not valid UTF-8 {}: {error}",
            path.display()
        )
    })
}

pub fn validate_executable_name(executable: &Path) -> Result<(), String> {
    validate_executable_name_for(executable, CANONICAL_BINARY_NAME)
}

pub fn validate_executable_name_for(
    executable: &Path,
    expected_binary: &str,
) -> Result<(), String> {
    let name = executable
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            format!(
                "executable path has no UTF-8 filename: {}",
                executable.display()
            )
        })?;
    let matches_expected =
        name == expected_binary || (cfg!(windows) && name == format!("{expected_binary}.exe"));
    if !matches_expected {
        return Err(format!(
            "metadata mode requires executable filename {expected_binary}, got {name}"
        ));
    }
    Ok(())
}

pub fn validate_metadata(metadata: &BuildMetadata, executable_hash: &str) -> Result<(), String> {
    validate_metadata_for(metadata, executable_hash, CANONICAL_BINARY_NAME)
}

pub fn validate_metadata_for(
    metadata: &BuildMetadata,
    executable_hash: &str,
    expected_binary: &str,
) -> Result<(), String> {
    validate_metadata_contract(
        metadata,
        executable_hash,
        expected_binary,
        "diagnostic-only",
        "octessera-hal",
        "orange-pi-zero-2w",
        "diagnostic-only",
    )
}

pub fn validate_runtime_candidate_metadata(
    metadata: &BuildMetadata,
    executable_hash: &str,
) -> Result<(), String> {
    validate_metadata_contract(
        metadata,
        executable_hash,
        RUNTIME_CANDIDATE_BINARY_NAME,
        "runtime-candidate",
        "octessera-pi",
        "hardware-orange-pi-zero-2w",
        "runtime-candidate",
    )
}

fn validate_metadata_contract(
    metadata: &BuildMetadata,
    executable_hash: &str,
    expected_binary: &str,
    expected_artifact_kind: &str,
    expected_package: &str,
    expected_cargo_feature: &str,
    contract_name: &str,
) -> Result<(), String> {
    if metadata.schema_version != METADATA_SCHEMA_VERSION
        || metadata.board_profile != "orange-pi-zero-2w"
        || metadata.artifact_kind != expected_artifact_kind
        || metadata.runtime_ready
        || metadata.binary != expected_binary
        || metadata.package != expected_package
        || metadata.arch != "aarch64-unknown-linux-gnu"
        || metadata.cargo_feature != expected_cargo_feature
        || !matches!(metadata.profile.as_str(), "release" | "pi-dev" | "dev")
    {
        return Err(format!(
            "metadata identity fields do not match the Orange {contract_name} contract"
        ));
    }
    if !is_lower_hex_sha256(&metadata.binary_sha256) {
        return Err("metadata binary_sha256 must be 64 lowercase hexadecimal characters".into());
    }
    if !is_lower_hex_sha256(executable_hash) {
        return Err("executable SHA-256 is not canonical lowercase hexadecimal".into());
    }
    if metadata.binary_sha256 != executable_hash {
        return Err("metadata binary_sha256 does not match the executable".into());
    }
    Ok(())
}

pub fn hash_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex_digest(hasher.finalize()))
}

fn hash_running_executable() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    {
        hash_file(Path::new("/proc/self/exe"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err("metadata mode requires Linux /proc/self/exe".into())
    }
}

fn hex_digest(digest: sha2::digest::Output<Sha256>) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn is_lower_hex_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
#[path = "orange_metadata_tests.rs"]
mod tests;
