use playback_runtime::{
    decode_user_data_bundle, encode_user_data_bundle, new_user_data_bundle,
    preference_delta_from_config, UserDataBundle, UserDataBundleMetadata, UserDataMediaKind,
    UserDataMediaReference, UserDataMusicalState, UserDataPreset, USER_DATA_MAX_BUNDLE_BYTES,
    USER_DATA_MAX_MEDIA_BYTES, USER_DATA_MAX_MEDIA_REFERENCES, USER_DATA_MAX_TOTAL_MEDIA_BYTES,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

#[path = "user_data_archive_media.rs"]
mod media;
use media::{
    directory_media, kind_name, media_key, media_kind_byte, media_kind_from_byte, media_stage_dir,
    sample_media,
};

const MAGIC: &[u8] = b"OCTESSERA-USER-DATA\0";
const ARCHIVE_VERSION: u32 = 1;
const ARCHIVE_OVERHEAD: u64 = 64 * 1024;

#[derive(Clone)]
pub(crate) struct MediaFile {
    pub(crate) reference: UserDataMediaReference,
    pub(crate) path: PathBuf,
}

pub(crate) struct ExportPlan {
    pub(crate) bundle: UserDataBundle,
    pub(crate) media: Vec<MediaFile>,
}

pub(crate) struct StagedMedia {
    pub(crate) reference: UserDataMediaReference,
    pub(crate) path: PathBuf,
}

pub(crate) struct StagedRestore {
    pub(crate) bundle: UserDataBundle,
    pub(crate) media: Vec<StagedMedia>,
    pub(crate) root: PathBuf,
}

pub(crate) fn canonical_defaults() -> Value {
    serde_json::from_str(include_str!("../../../config/generated/pi/default.json"))
        .expect("checked-in Pi default config is valid JSON")
}

pub(crate) fn max_archive_bytes() -> u64 {
    USER_DATA_MAX_BUNDLE_BYTES as u64
        + USER_DATA_MAX_TOTAL_MEDIA_BYTES
        + ARCHIVE_OVERHEAD
        + (USER_DATA_MAX_MEDIA_REFERENCES as u64 * 128)
}

pub(crate) fn build_export_plan(
    store_dir: &Path,
    samples_dir: &Path,
    recordings_dir: &Path,
    screen_recordings_dir: &Path,
    include_media: bool,
) -> Result<ExportPlan, String> {
    let canonical = canonical_defaults();
    let default_state = read_config_or_default(&store_dir.join("default.json"), &canonical)?;
    let current_state = first_existing_config(
        store_dir,
        &["current.json", "recovery-save.json", "default.json"],
        &canonical,
    )?;
    let presets = crate::platform_service::list_presets(store_dir)?
        .into_iter()
        .map(|display_name| {
            let path = crate::platform_service::preset_load_path(store_dir, &display_name)?;
            let patch = crate::platform_service::load_json(&path)?
                .ok_or_else(|| format!("preset `{display_name}` disappeared during export"))?;
            Ok(UserDataPreset {
                display_name,
                patch,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let preferences = preference_delta_from_config(&current_state, &canonical)?;
    let media = if include_media {
        let mut media = sample_media(samples_dir)?;
        media.extend(directory_media(
            recordings_dir,
            UserDataMediaKind::Audio,
            "audio recording",
        )?);
        media.extend(directory_media(
            screen_recordings_dir,
            UserDataMediaKind::Screen,
            "screen recording",
        )?);
        media
    } else {
        Vec::new()
    };
    let references = media
        .iter()
        .map(|media| media.reference.clone())
        .collect::<Vec<_>>();
    let bundle = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: crate::board_profile::BOARD_PROFILE_ID.into(),
            runtime_version: env!("CARGO_PKG_VERSION").into(),
        },
        presets,
        UserDataMusicalState {
            patch: current_state,
        },
        UserDataMusicalState {
            patch: default_state,
        },
        preferences,
        include_media,
        references,
        &canonical,
    )?;
    let mut media = media;
    media.sort_by(|left, right| media_key(&left.reference).cmp(&media_key(&right.reference)));
    Ok(ExportPlan { bundle, media })
}

pub(crate) fn archive_len(plan: &ExportPlan) -> Result<u64, String> {
    let bundle = encode_user_data_bundle(&plan.bundle)?;
    let mut length = (MAGIC.len() + 4 + 8 + bundle.len() + 4) as u64;
    for media in &plan.media {
        length = length
            .checked_add(1 + 2 + media.reference.id.len() as u64 + 8 + media.reference.size)
            .ok_or_else(|| "user-data archive size overflowed".to_string())?;
    }
    if length > max_archive_bytes() {
        return Err("user-data archive exceeds its size limit".into());
    }
    Ok(length)
}

pub(crate) fn write_archive<W: Write>(plan: &ExportPlan, writer: &mut W) -> Result<u64, String> {
    let bundle = encode_user_data_bundle(&plan.bundle)?;
    let length = archive_len(plan)?;
    writer.write_all(MAGIC).map_err(io_error)?;
    writer
        .write_all(&ARCHIVE_VERSION.to_le_bytes())
        .map_err(io_error)?;
    write_u64(writer, bundle.len() as u64)?;
    writer.write_all(&bundle).map_err(io_error)?;
    writer
        .write_all(&(plan.media.len() as u32).to_le_bytes())
        .map_err(io_error)?;
    for media in &plan.media {
        let id = media.reference.id.as_bytes();
        if id.len() > u16::MAX as usize {
            return Err("user-data media id is too long".into());
        }
        writer
            .write_all(&media_kind_byte(&media.reference.kind).to_le_bytes())
            .map_err(io_error)?;
        writer
            .write_all(&(id.len() as u16).to_le_bytes())
            .map_err(io_error)?;
        writer.write_all(id).map_err(io_error)?;
        write_u64(writer, media.reference.size)?;
        let mut file = File::open(&media.path).map_err(io_error)?;
        copy_exact(&mut file, writer, media.reference.size)?;
    }
    Ok(length)
}

pub(crate) fn stage_archive(
    archive_path: &Path,
    stage_root: &Path,
) -> Result<StagedRestore, String> {
    let size = fs::metadata(archive_path).map_err(io_error)?.len();
    if size > max_archive_bytes() {
        return Err("user-data archive exceeds its size limit".into());
    }
    if !stage_root.exists() {
        fs::create_dir_all(stage_root).map_err(io_error)?;
    }
    validate_stage_directory(stage_root)?;
    for directory in ["samples", "audio", "screen"] {
        let media_root = stage_root.join(directory);
        if !media_root.exists() {
            fs::create_dir(&media_root).map_err(io_error)?;
        }
        validate_stage_directory(&media_root)?;
    }
    let mut reader = File::open(archive_path).map_err(io_error)?;
    let mut magic = vec![0; MAGIC.len()];
    reader.read_exact(&mut magic).map_err(io_error)?;
    if magic != MAGIC {
        return Err("user-data archive magic is invalid".into());
    }
    if read_u32(&mut reader)? != ARCHIVE_VERSION {
        return Err("unsupported user-data archive version".into());
    }
    let bundle_size = read_u64(&mut reader)?;
    if bundle_size > USER_DATA_MAX_BUNDLE_BYTES as u64 {
        return Err("user-data bundle exceeds its size limit".into());
    }
    let mut bundle_bytes = vec![0; bundle_size as usize];
    reader.read_exact(&mut bundle_bytes).map_err(io_error)?;
    let bundle = decode_user_data_bundle(&bundle_bytes, &canonical_defaults())?;
    let media_count = read_u32(&mut reader)? as usize;
    if media_count > USER_DATA_MAX_MEDIA_REFERENCES || media_count != bundle.media.len() {
        return Err("user-data archive media count is invalid".into());
    }
    let mut seen = BTreeSet::new();
    let mut media = Vec::with_capacity(media_count);
    let mut total_bytes = 0_u64;
    for _ in 0..media_count {
        let kind = media_kind_from_byte(read_u8(&mut reader)?)?;
        let id_size = read_u16(&mut reader)? as usize;
        if id_size == 0 || id_size > 4 * 96 {
            return Err("user-data media id is too long".into());
        }
        let mut id_bytes = vec![0; id_size];
        reader.read_exact(&mut id_bytes).map_err(io_error)?;
        let id = String::from_utf8(id_bytes)
            .map_err(|_| "user-data media id is not UTF-8".to_string())?;
        let reference = bundle
            .media
            .iter()
            .find(|reference| reference.kind == kind && reference.id == id)
            .cloned()
            .ok_or_else(|| "user-data archive contains an unexpected media item".to_string())?;
        if !seen.insert((kind_name(&kind), id.clone())) {
            return Err("user-data archive contains duplicate media".into());
        }
        let declared_size = read_u64(&mut reader)?;
        if declared_size != reference.size || declared_size > USER_DATA_MAX_MEDIA_BYTES {
            return Err("user-data media size does not match its manifest".into());
        }
        total_bytes = total_bytes
            .checked_add(declared_size)
            .ok_or_else(|| "user-data media size total overflowed".to_string())?;
        if total_bytes > USER_DATA_MAX_TOTAL_MEDIA_BYTES {
            return Err("user-data media exceeds its size limit".into());
        }
        let path = stage_root.join(media_stage_dir(&kind)).join(&id);
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(io_error)?;
        let hash = copy_hashed(&mut reader, &mut output, declared_size)?;
        output.sync_all().map_err(io_error)?;
        if hash != reference.sha256 {
            return Err("user-data media hash does not match its manifest".into());
        }
        media.push(StagedMedia { reference, path });
    }
    if seen.len() != bundle.media.len() {
        return Err("user-data archive is missing media".into());
    }
    if reader.read(&mut [0; 1]).map_err(io_error)? != 0 {
        return Err("user-data archive has trailing bytes".into());
    }
    Ok(StagedRestore {
        bundle,
        media,
        root: stage_root.to_path_buf(),
    })
}

fn first_existing_config(
    store_dir: &Path,
    names: &[&str],
    canonical: &Value,
) -> Result<Value, String> {
    for name in names {
        let path = store_dir.join(name);
        if path.is_file() {
            return read_config_or_default(&path, canonical);
        }
    }
    Ok(canonical.clone())
}

fn read_config_or_default(path: &Path, canonical: &Value) -> Result<Value, String> {
    crate::platform_service::load_json(path).map(|value| value.unwrap_or_else(|| canonical.clone()))
}

fn copy_exact<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    mut remaining: u64,
) -> Result<(), String> {
    let mut buffer = [0; 16 * 1024];
    while remaining > 0 {
        let amount = (remaining as usize).min(buffer.len());
        let read = reader.read(&mut buffer[..amount]).map_err(io_error)?;
        if read == 0 {
            return Err("user-data media payload is truncated".into());
        }
        writer.write_all(&buffer[..read]).map_err(io_error)?;
        remaining -= read as u64;
    }
    Ok(())
}

fn copy_hashed<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    mut remaining: u64,
) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    while remaining > 0 {
        let amount = (remaining as usize).min(buffer.len());
        let read = reader.read(&mut buffer[..amount]).map_err(io_error)?;
        if read == 0 {
            return Err("user-data media payload is truncated".into());
        }
        writer.write_all(&buffer[..read]).map_err(io_error)?;
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn write_u64<W: Write>(writer: &mut W, value: u64) -> Result<(), String> {
    writer.write_all(&value.to_le_bytes()).map_err(io_error)
}

fn read_u8<R: Read>(reader: &mut R) -> Result<u8, String> {
    let mut value = [0; 1];
    reader.read_exact(&mut value).map_err(io_error)?;
    Ok(value[0])
}

fn read_u16<R: Read>(reader: &mut R) -> Result<u16, String> {
    let mut value = [0; 2];
    reader.read_exact(&mut value).map_err(io_error)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32<R: Read>(reader: &mut R) -> Result<u32, String> {
    let mut value = [0; 4];
    reader.read_exact(&mut value).map_err(io_error)?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64<R: Read>(reader: &mut R) -> Result<u64, String> {
    let mut value = [0; 8];
    reader.read_exact(&mut value).map_err(io_error)?;
    Ok(u64::from_le_bytes(value))
}

fn io_error(error: io::Error) -> String {
    error.to_string()
}

fn validate_stage_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("user-data staging path is unsafe".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "user_data_archive_tests.rs"]
mod tests;
