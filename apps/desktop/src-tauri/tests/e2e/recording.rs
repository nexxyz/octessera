//! The device recorder driven from System > Recording during playback: a WAV take, then an
//! Audio+Video take, each checked against the audio the session rendered over the same span.
use crate::support::{session, session_root};
use octessera_desktop_lib::headless::{HeadlessSession, Pace, PlayedScenario};
use playback_runtime::scenario::{press_main, turn, BAR};
use std::path::{Path, PathBuf};

/// AVI audio advances one 20 fps video frame every 2,205 frames.
const AUDIO_FRAMES_PER_VIDEO_FRAME: usize = 2_205;
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

/// RIFF chunks under `bytes` (after a form type), with LIST chunks opened up in place.
fn riff_chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut chunks = Vec::new();
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let id: [u8; 4] = bytes[at..at + 4].try_into().unwrap();
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        if &id == b"LIST" {
            chunks.extend(riff_chunks(&body[4..]));
        } else {
            chunks.push((id, body));
        }
        at += 8 + size + size % 2;
    }
    chunks
}

fn riff_form<'a>(bytes: &'a [u8], form: &[u8; 4]) -> Vec<([u8; 4], &'a [u8])> {
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8,
        "RIFF size"
    );
    assert_eq!(&bytes[8..12], form);
    riff_chunks(&bytes[12..])
}

/// 16-bit stereo 44.1 kHz PCM, as both formats carry it.
fn assert_pcm_format(format: &[u8]) {
    let field = |at: usize| u16::from_le_bytes(format[at..at + 2].try_into().unwrap());
    assert_eq!(
        (field(0), field(2), field(14)),
        (1, 2, 16),
        "PCM, stereo, 16 bit"
    );
    assert_eq!(u32::from_le_bytes(format[4..8].try_into().unwrap()), 44_100);
}

fn pcm(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_le_bytes(*pair))
        .collect()
}

fn wav_samples(bytes: &[u8]) -> Vec<i16> {
    let chunks = riff_form(bytes, b"WAVE");
    let chunk = |id: &[u8; 4]| chunks.iter().find(|(found, _)| found == id).unwrap().1;
    assert_pcm_format(chunk(b"fmt "));
    pcm(chunk(b"data"))
}

fn avi_samples(bytes: &[u8]) -> Vec<i16> {
    let chunks = riff_form(bytes, b"AVI ");
    let formats: Vec<_> = chunks.iter().filter(|(id, _)| id == b"strf").collect();
    assert_eq!(formats.len(), 2, "one video and one audio stream");
    assert_pcm_format(formats[1].1);
    let frames: Vec<_> = chunks.iter().filter(|(id, _)| id == b"00dc").collect();
    assert!(
        frames.iter().all(|(_, png)| png.starts_with(PNG_SIGNATURE)),
        "video frames are PNGs"
    );
    let samples: Vec<i16> = chunks
        .iter()
        .filter(|(id, _)| id == b"01wb")
        .flat_map(|(_, bytes)| pcm(bytes))
        .collect();
    let expected_frames = (samples.len() / 2).div_ceil(AUDIO_FRAMES_PER_VIDEO_FRAME);
    assert!(
        frames.len().abs_diff(expected_frames) <= 1,
        "{} video frames for {} audio frames",
        frames.len(),
        samples.len() / 2
    );
    samples
}

/// The one finished file in `dir`, refusing `.partial`/`.incomplete` leftovers.
fn only_file(dir: &Path, extension: &str) -> PathBuf {
    let files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("{dir:?}: {error}"))
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1, "{files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().to_string();
    assert!(
        name.ends_with(&format!(".{extension}"))
            && !name.contains(".partial.")
            && !name.contains(".incomplete."),
        "{name}"
    );
    files[0].clone()
}

/// Frame at which the `index`-th timed input ran.
fn cue_frame(session: &HeadlessSession, played: &PlayedScenario, index: usize) -> usize {
    played.start_frame
        + (played.observations[index].0 * f64::from(session.sample_rate())).round() as usize
}

/// Checks `recorded` is the session's audio from `from` to `to`, as 16-bit PCM.
fn assert_matches_session(
    session: &HeadlessSession,
    recorded: &[i16],
    (from, to): (usize, usize),
    take: &str,
) {
    let frames = recorded.len() / 2;
    assert!(
        frames.abs_diff(to - from) <= 1,
        "{take}: {frames} frames recorded for a {} frame span",
        to - from
    );
    let rendered = &session.audio()[from * 2..from * 2 + recorded.len()];
    let mismatched = rendered
        .iter()
        .zip(recorded)
        .filter(|(sample, pcm)| {
            let expected = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
            expected.abs_diff(**pcm) > 1
        })
        .count();
    assert_eq!(
        mismatched, 0,
        "{take}: samples differ from the session audio"
    );
    let peak = recorded.iter().map(|pcm| pcm.unsigned_abs()).max().unwrap();
    assert!(peak > 1_000, "{take}: near-silent take (peak {peak})");
}

#[test]
fn recorder_writes_wav_and_avi_takes_that_match_the_rendered_audio() {
    let mut take = crate::support::builder();
    take.open(&["System", "Recording"]);
    take.select("Start Audio");
    take.checkpoint("recording menu");
    let cues = [
        (BAR / 4, press_main()),
        (3 * BAR, turn(1)),
        (3 * BAR + 1, turn(1)),
        (3 * BAR + 2, press_main()),
        (4 * BAR, turn(-1)),
        (4 * BAR + 1, press_main()),
        (7 * BAR, turn(1)),
        (7 * BAR + 1, press_main()),
    ];
    for (pulse, input) in cues {
        take.at(pulse, false, input);
    }
    let mut session = session("recording", Pace::Fast);
    let played = session.play_scenario(&take.build(), 0.0, 15.0).unwrap();
    session.settle().unwrap();
    let wav_span = (
        cue_frame(&session, &played, 0),
        cue_frame(&session, &played, 3),
    );
    let avi_span = (
        cue_frame(&session, &played, 5),
        cue_frame(&session, &played, 7),
    );
    assert_eq!(session.recording_started_frame(), Some(wav_span.0));

    let media = session_root("recording").join("media");
    let wav = only_file(&media.join("recordings"), "wav");
    let wav_audio = wav_samples(&std::fs::read(&wav).unwrap());
    assert_matches_session(&session, &wav_audio, wav_span, "WAV");
    let avi = only_file(&media.join("screen-recordings"), "avi");
    let avi_audio = avi_samples(&std::fs::read(&avi).unwrap());
    assert_matches_session(&session, &avi_audio, avi_span, "AVI");
}
