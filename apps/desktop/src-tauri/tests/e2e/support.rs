use octessera_desktop_lib::headless::{HeadlessSession, Pace, PlayedEvent, PlayedScenario};
use playback_runtime::scenario::{ScenarioBuilder, BAR};
use std::path::PathBuf;

pub const DEFAULT_PATCH: &str =
    include_str!("../../../../../config/generated/desktop/default.json");

/// A fresh store and recordings folder for one test run.
pub fn session(name: &str, pace: Pace) -> HeadlessSession {
    let root = std::env::temp_dir()
        .join("octessera-e2e")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    HeadlessSession::start(root.join("store"), root.join("media"), pace)
        .unwrap_or_else(|error| panic!("{name}: session failed to start: {error}"))
}

pub fn builder() -> ScenarioBuilder {
    ScenarioBuilder::from_payload(
        octessera_desktop_lib::headless::desktop_runner_config(),
        serde_json::from_str(DEFAULT_PATCH).unwrap(),
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

/// Bar position (0-based, may be fractional) of a time in the take.
pub struct Timeline<'a> {
    pub session: &'a HeadlessSession,
    pub played: PlayedScenario,
}

impl Timeline<'_> {
    fn frame_at(&self, bar: f64) -> usize {
        self.played.start_frame + (bar * 2.0 * f64::from(self.session.sample_rate())) as usize
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
