//! A momentary Play FX mapped from the menu and held on its cell during playback: a filter
//! sweep at its default cutoff, compared with the same take played dry.
use crate::support::{assert_clean_audio, builder, keep_audio, session};
use octessera_desktop_lib::headless::{HeadlessSession, Pace, PlayedScenario};
use playback_runtime::scenario::{grid, BAR};
use std::path::PathBuf;

const FX_CELL: (usize, usize) = (3, 3);
const PRESS_BAR: u64 = 4;
const RELEASE_BAR: u64 = 6;
/// One bar at 120 BPM, in seconds.
const BAR_SECONDS: f64 = 2.0;

struct Take {
    session: HeadlessSession,
    played: PlayedScenario,
    wav: PathBuf,
}

impl Take {
    fn frames(&self, from_bar: f64, to_bar: f64) -> std::ops::Range<usize> {
        let at = |bar: f64| {
            self.played.start_frame
                + (bar * BAR_SECONDS * f64::from(self.session.sample_rate())) as usize
        };
        at(from_bar)..at(to_bar)
    }

    fn mono(&self, frame: usize) -> f64 {
        let audio = self.session.audio();
        f64::from(audio[frame * 2] + audio[frame * 2 + 1]) * 0.5
    }

    /// Level of the sample-to-sample difference, a crude high-pass, in dBFS.
    fn treble_db(&self, from_bar: f64, to_bar: f64) -> f64 {
        let frames = self.frames(from_bar, to_bar);
        let len = frames.len() as f64;
        let energy: f64 = frames
            .map(|frame| (self.mono(frame) - self.mono(frame - 1)).powi(2))
            .sum();
        10.0 * (energy / len + 1e-18).log10()
    }

    /// Level of the difference between this take and `other`, in dBFS.
    fn difference_db(&self, other: &Take, from_bar: f64, to_bar: f64) -> f64 {
        let frames = self.frames(from_bar, to_bar);
        let len = frames.len() as f64;
        let energy: f64 = frames
            .map(|frame| (self.mono(frame) - other.mono(frame)).powi(2))
            .sum();
        10.0 * (energy / len + 1e-18).log10()
    }
}

/// The default patch with a filter sweep mapped to `FX_CELL`, optionally held from `PRESS_BAR`
/// to `RELEASE_BAR`.
fn play(hold: bool) -> Take {
    let mut take = builder();
    take.open(&["Play", "FX"]);
    take.set_value("FX Type", "play.fx.type", "filter_sweep");
    take.set_number("Sweep In", "play.fx.params.sweepInMs", 10);
    take.set_number("Sweep Out", "play.fx.params.sweepOutMs", 10);
    take.enter("Map to Grid");
    take.tap_now(FX_CELL.0, FX_CELL.1);
    take.checkpoint("fx mapped");
    if hold {
        take.at(PRESS_BAR * BAR, true, grid(FX_CELL.0, FX_CELL.1, true));
        take.at(RELEASE_BAR * BAR, true, grid(FX_CELL.0, FX_CELL.1, false));
    }
    let name = format!("play-fx-{}", if hold { "held" } else { "dry" });
    let mut session = session(&name, Pace::Fast);
    let played = session.play_scenario(&take.build(), 0.0, 16.0).unwrap();
    let wav = keep_audio(&session, &name);
    assert_clean_audio(&session, &name);
    Take {
        session,
        played,
        wav,
    }
}

#[test]
fn held_play_fx_cell_filters_only_while_held() {
    let dry = play(false);
    let held = play(true);
    assert_eq!(held.played.start_frame, dry.played.start_frame);
    let (press, release) = (PRESS_BAR as f64, RELEASE_BAR as f64);
    let wavs = format!("audio in {:?} and {:?}", held.wav, dry.wav);

    // Cues land a pulse ahead of their bar line; the 10 ms sweeps fit inside 0.02 bar (40 ms).
    let silent_difference = |from: f64, to: f64| held.difference_db(&dry, from, to) < -120.0;
    assert!(
        silent_difference(0.0, press - 0.02),
        "the takes differ before the press, {wavs}"
    );
    let (wet, plain) = (
        held.treble_db(press + 0.02, release - 0.02),
        dry.treble_db(press + 0.02, release - 0.02),
    );
    assert!(
        wet < plain - 8.0,
        "held: treble {wet:.1} dB against {plain:.1} dB dry, {wavs}"
    );
    assert!(
        silent_difference(release + 0.02, 8.0),
        "the dry sound is not back after the release, {wavs}"
    );
}
