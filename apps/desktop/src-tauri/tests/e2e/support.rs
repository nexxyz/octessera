use octessera_desktop_lib::headless::{HeadlessSession, PlayedEvent, PlayedScenario};
use playback_runtime::scenario::{ScenarioBuilder, BAR};
use serde_json::Value;
use std::path::PathBuf;

/// Pulses per beat.
pub const BEAT: u64 = 24;

/// The patch the feature scenarios play: the factory patch as it was before the 0.9 factory
/// presets, kept as a fixture so a new shipped patch does not move every scenario's ground.
pub const SCENARIO_PATCH: &str = include_str!("fixtures/scenario_patch.json");

pub const SHIPPED_DEFAULT: &str =
    include_str!("../../../../../config/generated/desktop/default.json");

/// The folder holding one test run's store and its `media` (recordings) folder.
pub fn session_root(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join("octessera-e2e")
        .join(format!("{name}-{}", std::process::id()))
}

/// A fresh store, seeded with the scenario patch, and recordings folder for one test run.
pub fn session(name: &str) -> HeadlessSession {
    let root = session_root(name);
    let _ = std::fs::remove_dir_all(&root);
    seed_scenario_store(&root);
    start(name, root)
}

/// Writes the scenario patch into `root`'s empty store, so a session started there boots it.
pub fn seed_scenario_store(root: &std::path::Path) {
    let store = root.join("store");
    let documents = playback_runtime::split_system_patch_documents(
        &serde_json::from_str(SCENARIO_PATCH).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(&store).unwrap();
    for (file, document) in [
        ("system.json", &documents.system),
        ("default.patch.json", &documents.patch),
    ] {
        std::fs::write(
            store.join(file),
            serde_json::to_vec_pretty(document).unwrap(),
        )
        .unwrap();
    }
}

/// A fresh store that seeds itself with the shipped default patch, as a new install does.
pub fn shipped_session(name: &str) -> HeadlessSession {
    let root = session_root(name);
    let _ = std::fs::remove_dir_all(&root);
    start(name, root)
}

fn start(name: &str, root: PathBuf) -> HeadlessSession {
    HeadlessSession::start(root.join("store"), root.join("media"))
        .unwrap_or_else(|error| panic!("{name}: session failed to start: {error}"))
}

pub fn builder() -> ScenarioBuilder {
    builder_for(SCENARIO_PATCH)
}

pub fn shipped_builder() -> ScenarioBuilder {
    builder_for(SHIPPED_DEFAULT)
}

fn builder_for(patch: &str) -> ScenarioBuilder {
    ScenarioBuilder::from_payload(
        octessera_desktop_lib::headless::desktop_runner_config(),
        serde_json::from_str(patch).unwrap(),
    )
    .unwrap()
}

/// Writes the rendered audio next to the test temp files, for listening after a failure.
pub fn keep_audio(session: &HeadlessSession, name: &str) -> PathBuf {
    let path = std::env::temp_dir()
        .join("octessera-e2e")
        .join(format!("{name}.wav"));
    write_wav(&path, session.audio(), session.sample_rate());
    path
}

fn write_wav(path: &PathBuf, samples: &[f32], rate: u32) {
    let data: Vec<u8> = samples
        .iter()
        .flat_map(|sample| ((sample.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes())
        .collect();
    let mut bytes = Vec::with_capacity(44 + data.len());
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    for value in [16_u32, 0x0002_0001, rate, rate * 4, 0x0010_0004] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&data);
    let _ = std::fs::write(path, bytes);
}

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

/// The chunks of a RIFF file of type `form` (`WAVE`, `AVI `).
pub fn riff_form<'a>(bytes: &'a [u8], form: &[u8; 4]) -> Vec<([u8; 4], &'a [u8])> {
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8,
        "RIFF size"
    );
    assert_eq!(&bytes[8..12], form);
    riff_chunks(&bytes[12..])
}

/// The length of a 4/4 bar at the session's patch tempo, in seconds.
pub fn bar_seconds(session: &HeadlessSession) -> f64 {
    let bpm = session.config_payload()["runtimeConfig"]["transport"]["bpm"]
        .as_f64()
        .expect("a patch tempo");
    4.0 * 60.0 / bpm
}

/// Bar position (0-based, may be fractional) of a time in the take.
pub struct Timeline<'a> {
    pub session: &'a HeadlessSession,
    pub played: PlayedScenario,
}

impl Timeline<'_> {
    fn frame_at(&self, bar: f64) -> usize {
        self.played.start_frame
            + (bar * bar_seconds(self.session) * f64::from(self.session.sample_rate())) as usize
    }

    /// RMS level in dBFS between two bar positions.
    pub fn rms_db(&self, from_bar: f64, to_bar: f64) -> f64 {
        let audio = self.session.audio();
        let (a, b) = (
            self.frame_at(from_bar) * 2,
            (self.frame_at(to_bar) * 2).min(audio.len()),
        );
        let slice = &audio[a.min(b)..b];
        let mean = slice
            .iter()
            .map(|s| f64::from(*s) * f64::from(*s))
            .sum::<f64>()
            / slice.len().max(1) as f64;
        10.0 * (mean + 1e-18).log10()
    }

    /// Events whose pulse falls in `[from_bar, to_bar)`.
    pub fn events(&self, from_bar: f64, to_bar: f64) -> Vec<&PlayedEvent> {
        let from = self.played.start_pulse + (from_bar * BAR as f64) as u64;
        let to = self.played.start_pulse + (to_bar * BAR as f64) as u64;
        self.session
            .events()
            .iter()
            .filter(|event| event.pulse >= from && event.pulse < to)
            .collect()
    }

    pub fn slots(&self, from_bar: f64, to_bar: f64) -> Vec<u8> {
        let mut slots: Vec<u8> = self
            .events(from_bar, to_bar)
            .iter()
            .map(|e| e.slot)
            .collect();
        slots.sort_unstable();
        slots.dedup();
        slots
    }
}

pub fn assert_clean_audio(session: &HeadlessSession, name: &str) {
    let audio = session.audio();
    assert!(
        audio.iter().all(|s| s.is_finite()),
        "{name}: non-finite audio"
    );
    let peak = audio.iter().fold(0.0_f32, |peak, s| peak.max(s.abs()));
    assert!(peak < 1.0, "{name}: clips (peak {peak})");
}

/// Paths (and the two values) where `reloaded` differs from `saved`.
fn differences(saved: &Value, reloaded: &Value, path: String, out: &mut Vec<String>) {
    match (saved, reloaded) {
        (Value::Object(a), Value::Object(b)) => {
            for key in a.keys().chain(b.keys().filter(|key| !a.contains_key(*key))) {
                let (left, right) = (&a.get(key), &b.get(key));
                match (left, right) {
                    (Some(left), Some(right)) => {
                        differences(left, right, format!("{path}.{key}"), out);
                    }
                    _ => out.push(format!("{path}.{key}: {left:?} -> {right:?}")),
                }
            }
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => {
            for (index, (left, right)) in a.iter().zip(b).enumerate() {
                differences(left, right, format!("{path}[{index}]"), out);
            }
        }
        _ if saved != reloaded => out.push(format!("{path}: {saved} -> {reloaded}")),
        _ => {}
    }
}

/// Saved fields that came back differently after a restart (the save revision aside).
pub fn reload_differences(saved: &Value, reloaded: &Value) -> Vec<String> {
    let mut changed = Vec::new();
    differences(saved, reloaded, String::new(), &mut changed);
    changed.retain(|path| !path.starts_with(".revision:"));
    changed
}

/// Events whose pulse falls in `[from, to)`, for takes whose tempo changes.
pub fn events_between(session: &HeadlessSession, from: u64, to: u64) -> Vec<&PlayedEvent> {
    session
        .events()
        .iter()
        .filter(|event| event.pulse >= from && event.pulse < to)
        .collect()
}

/// Tempo heard between two pulses, from the frames of the events on its beats.
pub fn heard_bpm(session: &HeadlessSession, origin: u64, from: u64, to: u64) -> f64 {
    let beats: Vec<&PlayedEvent> = events_between(session, from, to)
        .into_iter()
        .filter(|event| (event.pulse - origin).is_multiple_of(BEAT))
        .collect();
    let (first, last) = (beats.first().unwrap(), beats.last().unwrap());
    assert!(
        last.pulse > first.pulse,
        "too few beats between {from} and {to}"
    );
    let frames_per_pulse = (last.frame - first.frame) as f64 / (last.pulse - first.pulse) as f64;
    60.0 * f64::from(session.sample_rate()) / (frames_per_pulse * BEAT as f64)
}

/// The frame of the first event at or after `pulse`.
pub fn frame_at_pulse(session: &HeadlessSession, pulse: u64) -> usize {
    session
        .events()
        .iter()
        .find(|event| event.pulse >= pulse)
        .map(|event| event.frame)
        .unwrap_or_else(|| panic!("no event from pulse {pulse} on"))
}

/// RMS level in dBFS between two frames.
pub fn rms_frames_db(session: &HeadlessSession, from_frame: usize, to_frame: usize) -> f64 {
    let slice = &session.audio()[from_frame * 2..to_frame * 2];
    let mean = slice
        .iter()
        .map(|s| f64::from(*s) * f64::from(*s))
        .sum::<f64>()
        / slice.len().max(1) as f64;
    10.0 * (mean + 1e-18).log10()
}

/// A grid LED's color in a snapshot, at world-space `x`, `y`.
pub fn led(snapshot: &Value, x: usize, y: usize) -> [u64; 3] {
    let index = platform_core::logical_to_display_index(x, y) * 3;
    let rgb = &snapshot["leds"]["rgb"];
    [0, 1, 2].map(|channel| rgb[index + channel].as_u64().unwrap())
}

pub fn bright_white(rgb: [u64; 3]) -> bool {
    rgb.iter().all(|channel| *channel >= 200)
}
