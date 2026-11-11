use super::*;
use crate::avi_writer::{AviWriter, FinishStats};
use crate::RecordingStatus;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "octessera-avi-repair-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn units(writer: &mut AviWriter, count: u8) {
    for unit in 0..count {
        let frame = vec![unit; 301];
        let audio = vec![i16::from(unit); 4_410];
        assert!(writer.write_unit(&frame, &audio).unwrap());
    }
}

fn writer_at(dir: &Path, stem: &str) -> AviWriter {
    let partial = dir.join(format!("{stem}.partial.avi"));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .unwrap();
    AviWriter::new(file, partial, dir.join(format!("{stem}.avi")), u64::MAX).unwrap()
}

fn finished_reference(dir: &Path, count: u8) -> Vec<u8> {
    let mut writer = writer_at(dir, "reference");
    units(&mut writer, count);
    let frames = writer.audio_frames;
    let outcome = writer
        .finish(FinishStats {
            frames,
            gap_count: 0,
            gap_frames: 0,
            overflow_count: 0,
            overflow_frames: 0,
            incomplete: false,
            size_limited: false,
        })
        .unwrap();
    let bytes = fs::read(&outcome.path).unwrap();
    fs::remove_file(outcome.path).unwrap();
    bytes
}

#[test]
fn a_power_cut_take_gets_its_sizes_and_index_back_up_to_the_last_whole_chunk() {
    let dir = temp_dir("torn-tail");
    let reference = finished_reference(&dir, 3);
    let mut writer = writer_at(&dir, "octessera-1");
    units(&mut writer, 3);
    drop(writer);
    let partial = dir.join("octessera-1.partial.avi");
    let mut torn = OpenOptions::new().append(true).open(&partial).unwrap();
    torn.write_all(b"00dc").unwrap();
    torn.write_all(&500_u32.to_le_bytes()).unwrap();
    torn.write_all(&[7; 120]).unwrap();
    drop(torn);
    assert_eq!(partial_avi_paths(&dir), vec![partial.clone()]);

    let outcome = repair_partial_avi(&partial).unwrap();

    assert_eq!(outcome.path, dir.join("octessera-1.incomplete.avi"));
    assert_eq!(outcome.frames_written, 3 * 2_205);
    assert!(matches!(outcome.status, RecordingStatus::Incomplete { .. }));
    assert!(!partial.exists());
    assert_eq!(fs::read(&outcome.path).unwrap(), reference);
    assert!(partial_avi_paths(&dir).is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_take_cut_inside_its_first_chunk_finalizes_empty() {
    let dir = temp_dir("first-chunk");
    let reference = finished_reference(&dir, 0);
    drop(writer_at(&dir, "octessera-2"));
    let partial = dir.join("octessera-2.partial.avi");
    let mut torn = OpenOptions::new().append(true).open(&partial).unwrap();
    torn.write_all(b"01w").unwrap();
    drop(torn);

    let outcome = repair_partial_avi(&partial).unwrap();

    assert_eq!(outcome.frames_written, 0);
    assert_eq!(fs::read(&outcome.path).unwrap(), reference);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn a_file_without_our_header_is_left_alone() {
    let dir = temp_dir("foreign");
    let partial = dir.join("octessera-3.partial.avi");
    File::create(&partial).unwrap().write_all(b"RIFF").unwrap();

    assert!(repair_partial_avi(&partial).is_err());

    assert_eq!(fs::read(&partial).unwrap(), b"RIFF");
    assert!(!dir.join("octessera-3.incomplete.avi").exists());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn only_partial_avi_takes_are_listed() {
    let dir = temp_dir("listing");
    for name in ["b.partial.avi", "a.partial.avi", "c.avi", "d.partial.wav"] {
        File::create(dir.join(name)).unwrap();
    }

    assert_eq!(
        partial_avi_paths(&dir),
        vec![dir.join("a.partial.avi"), dir.join("b.partial.avi")]
    );
    assert!(partial_avi_paths(&dir.join("missing")).is_empty());
    let _ = fs::remove_dir_all(dir);
}
