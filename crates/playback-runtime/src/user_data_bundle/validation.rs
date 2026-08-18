use super::{
    format, is_safe_user_data_name, preferences, UserDataBundle, UserDataBundleMetadata,
    UserDataManifestEntryKind, UserDataMediaReference, USER_DATA_BUNDLE_KIND,
    USER_DATA_BUNDLE_SCHEMA_VERSION, USER_DATA_MAX_MANIFEST_ENTRIES, USER_DATA_MAX_MEDIA_BYTES,
    USER_DATA_MAX_MEDIA_REFERENCES, USER_DATA_MAX_METADATA_CHARS, USER_DATA_MAX_PRESETS,
    USER_DATA_MAX_TOTAL_MEDIA_BYTES,
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub(super) fn structural(bundle: &UserDataBundle) -> Result<(), String> {
    if bundle.kind != USER_DATA_BUNDLE_KIND {
        return Err(format!(
            "unsupported user-data bundle kind `{}`",
            bundle.kind
        ));
    }
    if bundle.schema_version != USER_DATA_BUNDLE_SCHEMA_VERSION {
        return Err(format!(
            "unsupported user-data schema version {}",
            bundle.schema_version
        ));
    }
    metadata(&bundle.metadata)?;
    if bundle.presets.len() > USER_DATA_MAX_PRESETS {
        return Err("user-data bundle has too many presets".into());
    }
    if bundle.media.len() > USER_DATA_MAX_MEDIA_REFERENCES {
        return Err("user-data bundle has too many media references".into());
    }
    for preset in &bundle.presets {
        name(&preset.display_name, "preset display name")?;
    }
    unique_names(
        bundle
            .presets
            .iter()
            .map(|preset| preset.display_name.as_str()),
    )?;
    if !bundle.media_included && !bundle.media.is_empty() {
        return Err("media references require mediaIncluded=true".into());
    }
    let mut media_keys = BTreeSet::new();
    let mut total_media_bytes = 0u64;
    for reference in &bundle.media {
        media_reference(reference)?;
        media_keys.insert(format::media_sort_key(reference));
        total_media_bytes = total_media_bytes
            .checked_add(reference.size)
            .ok_or_else(|| "user-data media size total overflowed".to_string())?;
    }
    if media_keys.len() != bundle.media.len() {
        return Err("duplicate user-data media reference".into());
    }
    if total_media_bytes > USER_DATA_MAX_TOTAL_MEDIA_BYTES {
        return Err("user-data media exceeds its size limit".into());
    }
    preferences::shape(&bundle.preferences)?;
    manifest_shape(bundle)
}

pub(super) fn metadata(metadata: &UserDataBundleMetadata) -> Result<(), String> {
    metadata_text(&metadata.board_profile, "metadata.boardProfile")?;
    metadata_text(&metadata.runtime_version, "metadata.runtimeVersion")
}

pub(super) fn name(name: &str, path: &str) -> Result<(), String> {
    if !is_safe_user_data_name(name) {
        return Err(format!("{path} is unsafe or too long"));
    }
    Ok(())
}

pub(super) fn media_reference(reference: &UserDataMediaReference) -> Result<(), String> {
    name(&reference.id, "media id")?;
    name(&reference.display_name, "media display name")?;
    if reference.size > USER_DATA_MAX_MEDIA_BYTES || !format::is_sha256(&reference.sha256) {
        return Err(format!("media reference `{}` is invalid", reference.id));
    }
    Ok(())
}

pub(super) fn unique_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name) {
            return Err(format!("duplicate user-data name `{name}`"));
        }
    }
    Ok(())
}

pub(super) fn manifest_shape(bundle: &UserDataBundle) -> Result<(), String> {
    if bundle.manifest.len() > USER_DATA_MAX_MANIFEST_ENTRIES {
        return Err("user-data bundle manifest is too large".into());
    }
    for entry in &bundle.manifest {
        let valid_path = match entry.kind {
            UserDataManifestEntryKind::Metadata => entry.path == "metadata.json",
            UserDataManifestEntryKind::CurrentState => entry.path == "state/current.json",
            UserDataManifestEntryKind::DefaultState => entry.path == "state/default.json",
            UserDataManifestEntryKind::Preferences => entry.path == "preferences/delta.json",
            UserDataManifestEntryKind::Preset => entry
                .path
                .strip_prefix("presets/")
                .and_then(|path| path.strip_suffix(".json"))
                .is_some_and(|name| !name.contains('/') && is_safe_user_data_name(name)),
            UserDataManifestEntryKind::Media => {
                let mut parts = entry.path.split('/');
                matches!(parts.next(), Some("media"))
                    && matches!(parts.next(), Some("sample" | "audio" | "screen"))
                    && parts.next().is_some_and(is_safe_user_data_name)
                    && parts.next().is_none()
            }
        };
        if !valid_path {
            return Err(format!("manifest path `{}` is unsafe", entry.path));
        }
        let max_size = if entry.kind == UserDataManifestEntryKind::Media {
            USER_DATA_MAX_MEDIA_BYTES
        } else {
            super::USER_DATA_MAX_ITEM_BYTES as u64
        };
        if entry.size > max_size {
            return Err(format!("manifest entry `{}` is too large", entry.path));
        }
        if !format::is_sha256(&entry.sha256) {
            return Err(format!(
                "manifest entry `{}` has an invalid SHA-256",
                entry.path
            ));
        }
    }
    Ok(())
}

pub(super) fn runtime<'a>(
    payload: &'a Value,
    path: &str,
) -> Result<&'a Map<String, Value>, String> {
    payload
        .get("runtimeConfig")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{path}.runtimeConfig must be an object"))
}

fn metadata_text(value: &str, path: &str) -> Result<(), String> {
    if value.is_empty()
        || value.chars().count() > USER_DATA_MAX_METADATA_CHARS
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\'))
    {
        return Err(format!("{path} is invalid"));
    }
    Ok(())
}
