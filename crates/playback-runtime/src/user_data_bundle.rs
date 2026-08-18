use crate::is_valid_preset_name;
use crate::native_runner::{normalize_user_data_patch_payload, validate_user_data_config_payload};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

mod format;
mod migration;
mod preferences;
mod validation;

pub const USER_DATA_BUNDLE_KIND: &str = "octessera.user-data";
pub const USER_DATA_BUNDLE_SCHEMA_VERSION: u64 = 1;
pub const USER_DATA_MAX_BUNDLE_BYTES: usize = 4 * 1024 * 1024;
pub const USER_DATA_MAX_PRESETS: usize = 256;
pub const USER_DATA_MAX_PRESET_NAME_CHARS: usize = 96;
pub const USER_DATA_MAX_MEDIA_REFERENCES: usize = 256;
pub const USER_DATA_MAX_MANIFEST_ENTRIES: usize = 512;
pub const USER_DATA_MAX_ITEM_BYTES: usize = 512 * 1024;
pub const USER_DATA_MAX_METADATA_CHARS: usize = 96;
pub const USER_DATA_MAX_MEDIA_BYTES: u64 = 256 * 1024 * 1024;
pub const USER_DATA_MAX_TOTAL_MEDIA_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataBundleMetadata {
    #[serde(rename = "boardProfile")]
    pub board_profile: String,
    #[serde(rename = "runtimeVersion")]
    pub runtime_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataPreset {
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub patch: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataMusicalState {
    pub patch: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDataMediaKind {
    Sample,
    Audio,
    Screen,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataMediaReference {
    pub id: String,
    pub kind: UserDataMediaKind,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub size: u64,
    #[serde(rename = "sha256")]
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserDataManifestEntryKind {
    Metadata,
    Preset,
    CurrentState,
    DefaultState,
    Preferences,
    Media,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataManifestEntry {
    pub path: String,
    pub kind: UserDataManifestEntryKind,
    pub size: u64,
    #[serde(rename = "sha256")]
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPreferenceDelta {
    #[serde(flatten)]
    pub values: BTreeMap<String, Value>,
}

impl UserPreferenceDelta {
    pub fn empty() -> Self {
        Self {
            values: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDataBundle {
    pub kind: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: u64,
    pub metadata: UserDataBundleMetadata,
    pub manifest: Vec<UserDataManifestEntry>,
    pub presets: Vec<UserDataPreset>,
    #[serde(rename = "currentState")]
    pub current_state: UserDataMusicalState,
    #[serde(rename = "defaultState")]
    pub default_state: UserDataMusicalState,
    pub preferences: UserPreferenceDelta,
    #[serde(rename = "mediaIncluded")]
    pub media_included: bool,
    pub media: Vec<UserDataMediaReference>,
}

impl UserDataBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        metadata: UserDataBundleMetadata,
        presets: Vec<UserDataPreset>,
        current_state: UserDataMusicalState,
        default_state: UserDataMusicalState,
        preferences: UserPreferenceDelta,
        media_included: bool,
        media: Vec<UserDataMediaReference>,
        canonical_defaults: &Value,
    ) -> Result<Self, String> {
        new_user_data_bundle(
            metadata,
            presets,
            current_state,
            default_state,
            preferences,
            media_included,
            media,
            canonical_defaults,
        )
    }
}

pub fn is_safe_user_data_name(name: &str) -> bool {
    name.chars().count() <= USER_DATA_MAX_PRESET_NAME_CHARS && is_valid_preset_name(name)
}

#[allow(clippy::too_many_arguments)]
pub fn new_user_data_bundle(
    metadata: UserDataBundleMetadata,
    mut presets: Vec<UserDataPreset>,
    current_state: UserDataMusicalState,
    default_state: UserDataMusicalState,
    preferences: UserPreferenceDelta,
    media_included: bool,
    mut media: Vec<UserDataMediaReference>,
    canonical_defaults: &Value,
) -> Result<UserDataBundle, String> {
    if presets.len() > USER_DATA_MAX_PRESETS {
        return Err("user-data bundle has too many presets".into());
    }
    if media.len() > USER_DATA_MAX_MEDIA_REFERENCES {
        return Err("user-data bundle has too many media references".into());
    }
    validation::metadata(&metadata)?;
    let current_state = UserDataMusicalState {
        patch: normalize_patch(current_state.patch, canonical_defaults)?,
    };
    let default_state = UserDataMusicalState {
        patch: normalize_patch(default_state.patch, canonical_defaults)?,
    };
    for preset in &mut presets {
        validation::name(&preset.display_name, "preset display name")?;
        preset.patch = normalize_patch(preset.patch.clone(), canonical_defaults)?;
    }
    presets.sort_by(|left, right| left.display_name.cmp(&right.display_name));
    validation::unique_names(presets.iter().map(|preset| preset.display_name.as_str()))?;
    media.sort_by(|left, right| format::media_sort_key(left).cmp(&format::media_sort_key(right)));
    for reference in &media {
        validation::media_reference(reference)?;
    }
    let mut bundle = UserDataBundle {
        kind: USER_DATA_BUNDLE_KIND.into(),
        schema_version: USER_DATA_BUNDLE_SCHEMA_VERSION,
        metadata,
        manifest: Vec::new(),
        presets,
        current_state,
        default_state,
        preferences,
        media_included,
        media,
    };
    bundle.manifest = manifest_for_user_data_bundle(&bundle)?;
    validate_user_data_bundle(&bundle, canonical_defaults)?;
    Ok(bundle)
}

pub fn manifest_for_user_data_bundle(
    bundle: &UserDataBundle,
) -> Result<Vec<UserDataManifestEntry>, String> {
    validation::structural(bundle)?;
    format::manifest(bundle)
}

pub fn validate_user_data_bundle(
    bundle: &UserDataBundle,
    canonical_defaults: &Value,
) -> Result<(), String> {
    validation::structural(bundle)?;
    validate_user_data_config_payload(canonical_defaults)?;
    if bundle.manifest != manifest_for_user_data_bundle(bundle)? {
        return Err("user-data bundle manifest does not match its contents".into());
    }
    for (path, patch) in [
        ("currentState.patch", &bundle.current_state.patch),
        ("defaultState.patch", &bundle.default_state.patch),
    ] {
        validate_canonical_patch(patch, canonical_defaults, path)?;
    }
    for (index, preset) in bundle.presets.iter().enumerate() {
        validate_canonical_patch(
            &preset.patch,
            canonical_defaults,
            &format!("presets[{index}].patch"),
        )?;
    }
    apply_user_preference_delta(canonical_defaults, &bundle.preferences)?;
    if format::bundle_bytes(bundle)?.len() > USER_DATA_MAX_BUNDLE_BYTES {
        return Err("user-data bundle exceeds its size limit".into());
    }
    Ok(())
}

pub fn encode_user_data_bundle(bundle: &UserDataBundle) -> Result<Vec<u8>, String> {
    let mut canonical_bundle = bundle.clone();
    canonical_bundle
        .presets
        .sort_by(|left, right| left.display_name.cmp(&right.display_name));
    canonical_bundle
        .media
        .sort_by(|left, right| format::media_sort_key(left).cmp(&format::media_sort_key(right)));
    validation::structural(&canonical_bundle)?;
    if canonical_bundle.manifest != manifest_for_user_data_bundle(&canonical_bundle)? {
        return Err("user-data bundle manifest does not match its contents".into());
    }
    let bytes = format::bundle_bytes(&canonical_bundle)?;
    if bytes.len() > USER_DATA_MAX_BUNDLE_BYTES {
        return Err("user-data bundle exceeds its size limit".into());
    }
    Ok(bytes)
}

pub fn decode_user_data_bundle(
    bytes: &[u8],
    canonical_defaults: &Value,
) -> Result<UserDataBundle, String> {
    if bytes.len() > USER_DATA_MAX_BUNDLE_BYTES {
        return Err("user-data bundle exceeds its size limit".into());
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("user-data bundle JSON is invalid: {error}"))?;
    migrate_user_data_bundle(value, canonical_defaults)
}

pub fn migrate_user_data_bundle(
    mut value: Value,
    canonical_defaults: &Value,
) -> Result<UserDataBundle, String> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| "user-data bundle must be an object".to_string())?;
    let version = object
        .get("schemaVersion")
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| "user-data schemaVersion must be an integer".to_string())
        })
        .transpose()?
        .unwrap_or(0);
    if version > USER_DATA_BUNDLE_SCHEMA_VERSION {
        return Err(format!("unsupported user-data schema version {version}"));
    }
    if version == 0 {
        migration::legacy_fields(object)?;
    }
    let mut bundle: UserDataBundle = serde_json::from_value(value)
        .map_err(|error| format!("user-data bundle fields are invalid: {error}"))?;
    if version == USER_DATA_BUNDLE_SCHEMA_VERSION {
        validation::manifest_shape(&bundle)?;
        if bundle.manifest != manifest_for_user_data_bundle(&bundle)? {
            return Err("user-data bundle manifest does not match its contents".into());
        }
    }
    bundle.current_state.patch = normalize_patch(bundle.current_state.patch, canonical_defaults)?;
    bundle.default_state.patch = normalize_patch(bundle.default_state.patch, canonical_defaults)?;
    for preset in &mut bundle.presets {
        preset.patch = normalize_patch(preset.patch.clone(), canonical_defaults)?;
    }
    bundle
        .presets
        .sort_by(|left, right| left.display_name.cmp(&right.display_name));
    bundle
        .media
        .sort_by(|left, right| format::media_sort_key(left).cmp(&format::media_sort_key(right)));
    bundle.kind = USER_DATA_BUNDLE_KIND.into();
    bundle.schema_version = USER_DATA_BUNDLE_SCHEMA_VERSION;
    bundle.manifest = manifest_for_user_data_bundle(&bundle)?;
    validate_user_data_bundle(&bundle, canonical_defaults)?;
    Ok(bundle)
}

pub fn preference_delta_from_config(
    current: &Value,
    canonical_defaults: &Value,
) -> Result<UserPreferenceDelta, String> {
    validate_user_data_config_payload(current)?;
    validate_user_data_config_payload(canonical_defaults)?;
    let current_runtime = validation::runtime(current, "current")?;
    let default_runtime = validation::runtime(canonical_defaults, "canonical defaults")?;
    let current_values = preferences::projection(current_runtime);
    let defaults = preferences::projection(default_runtime);
    let delta = UserPreferenceDelta {
        values: current_values
            .into_iter()
            .filter(|(key, value)| defaults.get(key) != Some(value))
            .collect(),
    };
    preferences::validate(&delta, canonical_defaults)?;
    Ok(delta)
}

pub fn apply_user_preference_delta(
    canonical_defaults: &Value,
    delta: &UserPreferenceDelta,
) -> Result<Value, String> {
    validate_user_data_config_payload(canonical_defaults)?;
    preferences::validate(delta, canonical_defaults)?;
    preferences::apply(canonical_defaults, delta)
}

pub fn media_reference_from_bytes(
    kind: UserDataMediaKind,
    id: String,
    display_name: String,
    bytes: &[u8],
) -> Result<UserDataMediaReference, String> {
    let reference = UserDataMediaReference {
        id,
        kind,
        display_name,
        size: bytes.len() as u64,
        sha256: format::sha256_hex(bytes),
    };
    validation::media_reference(&reference)?;
    Ok(reference)
}

fn normalize_patch(patch: Value, canonical_defaults: &Value) -> Result<Value, String> {
    let normalized = normalize_user_data_patch_payload(patch, canonical_defaults)?;
    if format::json_bytes(&normalized)?.len() > USER_DATA_MAX_ITEM_BYTES {
        return Err("user-data musical state exceeds its size limit".into());
    }
    Ok(normalized)
}

fn validate_canonical_patch(
    patch: &Value,
    canonical_defaults: &Value,
    path: &str,
) -> Result<(), String> {
    let normalized = normalize_user_data_patch_payload(patch.clone(), canonical_defaults)
        .map_err(|error| format!("{path}: {error}"))?;
    if normalized != *patch {
        return Err(format!("{path} is not the canonical migrated patch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
