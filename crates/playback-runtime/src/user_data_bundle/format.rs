use super::{
    UserDataBundle, UserDataManifestEntry, UserDataManifestEntryKind, UserDataMediaKind,
    UserDataMediaReference, USER_DATA_MAX_ITEM_BYTES, USER_DATA_MAX_MANIFEST_ENTRIES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn manifest(bundle: &UserDataBundle) -> Result<Vec<UserDataManifestEntry>, String> {
    let mut entries = vec![
        json_entry(
            "metadata.json",
            UserDataManifestEntryKind::Metadata,
            serde_json::to_value(&bundle.metadata).map_err(|error| error.to_string())?,
        )?,
        json_entry(
            "state/current.json",
            UserDataManifestEntryKind::CurrentState,
            bundle.current_state.patch.clone(),
        )?,
        json_entry(
            "state/default.json",
            UserDataManifestEntryKind::DefaultState,
            bundle.default_state.patch.clone(),
        )?,
        json_entry(
            "preferences/delta.json",
            UserDataManifestEntryKind::Preferences,
            serde_json::to_value(&bundle.preferences).map_err(|error| error.to_string())?,
        )?,
    ];
    entries.extend(
        bundle
            .presets
            .iter()
            .map(|preset| {
                json_entry(
                    &format!("presets/{}.json", preset.display_name),
                    UserDataManifestEntryKind::Preset,
                    preset.patch.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    if bundle.media_included {
        entries.extend(bundle.media.iter().map(|reference| UserDataManifestEntry {
            path: format!(
                "media/{}/{}",
                media_kind_name(&reference.kind),
                reference.id
            ),
            kind: UserDataManifestEntryKind::Media,
            size: reference.size,
            sha256: reference.sha256.clone(),
        }));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    if entries.len() > USER_DATA_MAX_MANIFEST_ENTRIES {
        return Err("user-data bundle manifest is too large".into());
    }
    Ok(entries)
}

pub(super) fn bundle_bytes(bundle: &UserDataBundle) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(bundle).map_err(|error| error.to_string())?;
    json_bytes(&value)
}

pub(super) fn json_bytes(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&canonicalize(value.clone())).map_err(|error| error.to_string())
}

pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(super) fn media_sort_key(reference: &UserDataMediaReference) -> (&'static str, &str) {
    (media_kind_name(&reference.kind), reference.id.as_str())
}

fn json_entry(
    path: &str,
    kind: UserDataManifestEntryKind,
    value: Value,
) -> Result<UserDataManifestEntry, String> {
    let bytes = json_bytes(&value)?;
    if bytes.len() > USER_DATA_MAX_ITEM_BYTES {
        return Err(format!("manifest entry `{path}` is too large"));
    }
    Ok(UserDataManifestEntry {
        path: path.into(),
        kind,
        size: bytes.len() as u64,
        sha256: sha256_hex(&bytes),
    })
}

fn canonicalize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize).collect()),
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, canonicalize(value)))
                .collect::<BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        value => value,
    }
}

pub(super) fn media_kind_name(kind: &UserDataMediaKind) -> &'static str {
    match kind {
        UserDataMediaKind::Sample => "sample",
        UserDataMediaKind::Audio => "audio",
        UserDataMediaKind::Screen => "screen",
    }
}
