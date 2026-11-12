use super::avi_writer::{AviWriter, FinishStats};
use super::RecordingOutcome;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const PARTIAL_AVI_SUFFIX: &str = ".partial.avi";

/// The `.partial.avi` takes in `dir`; list them before any new take can start in it.
pub fn partial_avi_paths(dir: &Path) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(PARTIAL_AVI_SUFFIX))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

/// Finalizes a take a power cut left without sizes or index: keeps every whole movi chunk,
/// drops a torn tail, and saves it as `.incomplete.avi`, also when the cut came while `finish`
/// was patching the sizes. A file this recorder did not write is left untouched.
pub fn repair_partial_avi(partial_path: &Path) -> Result<RecordingOutcome, String> {
    let name = partial_path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(PARTIAL_AVI_SUFFIX))
        .ok_or_else(|| format!("{} is not a partial AVI take", partial_path.display()))?;
    let final_path = partial_path.with_file_name(format!("{name}.avi"));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(partial_path)
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    let mut writer = AviWriter::reopen(file, partial_path.to_path_buf(), final_path)
        .map_err(|error| error.to_string())?;
    index_whole_chunks(&mut writer)
        .map_err(|error| format!("{}: {error}", partial_path.display()))?;
    let frames = writer.audio_frames;
    writer
        .finish(FinishStats {
            frames,
            gap_count: 0,
            gap_frames: 0,
            overflow_count: 0,
            overflow_frames: 0,
            incomplete: true,
            size_limited: false,
        })
        .map_err(|error| error.to_string())
}

fn index_whole_chunks(writer: &mut AviWriter) -> std::io::Result<()> {
    let movi_fourcc = writer.movi_data_start() - 4;
    let mut position = writer.movi_data_start();
    let length = writer.file_mut().metadata()?.len();
    let mut header = [0_u8; 8];
    while position + 8 <= length {
        let file = writer.file_mut();
        file.seek(SeekFrom::Start(position))?;
        file.read_exact(&mut header)?;
        let id = [header[0], header[1], header[2], header[3]];
        let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
        let end = position + 8 + u64::from(size) + u64::from(size % 2);
        if !matches!(&id, b"00dc" | b"01wb") || end > length {
            break;
        }
        let Ok(offset) = u32::try_from(position - movi_fourcc) else {
            break;
        };
        writer.record_chunk(id, offset, size);
        position = end;
    }
    let file = writer.file_mut();
    file.set_len(position)?;
    file.seek(SeekFrom::Start(position))?;
    Ok(())
}

#[cfg(test)]
mod tests;
