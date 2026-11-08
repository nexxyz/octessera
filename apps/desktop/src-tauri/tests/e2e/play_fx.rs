//! Momentary Play FX mapped from the menu and held on their tile during playback, each take
//! compared with the same take played dry: a filter sweep at its default cutoff, and the seven
//! types that finish their release on their own, heard while held and gone after their tail.
use crate::support::{assert_clean_audio, bar_seconds, builder, keep_audio, session};
use octessera_desktop_lib::headless::{HeadlessSession, PlayedScenario};
use playback_runtime::scenario::{grid, ScenarioBuilder, BAR};
use std::path::PathBuf;

const FX_TILE: (usize, usize) = (3, 3);
const PRESS_BAR: u64 = 4;
const RELEASE_BAR: u64 = 6;
/// The types after `pitch_shift`, which run out their release by themselves.
const SELF_RELEASING_TYPES: [&str; 7] = [
    "tape_stop",
    "reverse",
    "trance_gate",
    "dub_throw",
    "wash",
    "hp_riser",
    "crush",
];

struct Take {
    session: HeadlessSession,
    played: PlayedScenario,
    wav: PathBuf,
}

impl Take {
    fn frames(&self, from_bar: f64, to_bar: f64) -> std::ops::Range<usize> {
        let frames_per_bar = bar_seconds(&self.session) * f64::from(self.session.sample_rate());
        let at = |bar: f64| self.played.start_frame + (bar * frames_per_bar) as usize;
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

/// The scenario patch with `fx_type` mapped to `FX_TILE` (after `edit` sets its rows), played
/// for `seconds` and held from `PRESS_BAR` to `RELEASE_BAR` when `hold`.
fn play(fx_type: &str, edit: impl FnOnce(&mut ScenarioBuilder), hold: bool, seconds: f64) -> Take {
    let mut take = builder();
    take.open(&["Play", "FX"]);
    take.set_value("FX Type", "play.fx.type", fx_type);
    edit(&mut take);
    take.enter("Map to Grid");
    take.tap_now(FX_TILE.0, FX_TILE.1);
    take.checkpoint("fx mapped");
    if hold {
        take.at(PRESS_BAR * BAR, true, grid(FX_TILE.0, FX_TILE.1, true));
        take.at(RELEASE_BAR * BAR, true, grid(FX_TILE.0, FX_TILE.1, false));
    }
    let name = format!("play-fx-{fx_type}-{}", if hold { "held" } else { "dry" });
    let mut session = session(&name);
    let played = session.play_scenario(&take.build(), 0.0, seconds).unwrap();
    let wav = keep_audio(&session, &name);
    assert_clean_audio(&session, &name);
    Take {
        session,
        played,
        wav,
    }
}

fn play_filter_sweep(hold: bool) -> Take {
    let edit = |take: &mut ScenarioBuilder| {
        take.set_number("Sweep In", "play.fx.params.sweepInMs", 10);
        take.set_number("Sweep Out", "play.fx.params.sweepOutMs", 10);
    };
    play("filter_sweep", edit, hold, 16.0)
}

#[test]
fn held_play_fx_tile_filters_only_while_held() {
    let dry = play_filter_sweep(false);
    let held = play_filter_sweep(true);
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

/// Dub throw's default Mix adds its echoes to the hot scenario mix up to full scale; Mix 40
/// keeps the take below it.
fn below_full_scale(fx_type: &str) -> impl FnOnce(&mut ScenarioBuilder) + '_ {
    move |take| {
        if fx_type == "dub_throw" {
            take.set_number("Mix", "play.fx.params.mixPct", 40);
        }
    }
}

#[test]
fn self_releasing_play_fx_are_heard_while_held_and_gone_after_their_tail() {
    const SECONDS: f64 = 30.0;
    let (press, release) = (PRESS_BAR as f64, RELEASE_BAR as f64);
    for fx_type in SELF_RELEASING_TYPES {
        let dry = play(fx_type, below_full_scale(fx_type), false, SECONDS);
        let held = play(fx_type, below_full_scale(fx_type), true, SECONDS);
        let end = SECONDS / bar_seconds(&dry.session);
        assert_eq!(held.played.start_frame, dry.played.start_frame, "{fx_type}");
        let wavs = format!("audio in {:?} and {:?}", held.wav, dry.wav);
        let difference = |from: f64, to: f64| held.difference_db(&dry, from, to);
        assert!(
            difference(0.0, press - 0.02) < -120.0,
            "{fx_type}: the takes differ before the press, {wavs}"
        );
        let heard = difference(press + 0.25, release - 0.02);
        assert!(
            heard > -40.0,
            "{fx_type}: held only {heard:.1} dB off dry, {wavs}"
        );
        let left = difference(end - 2.0, end - 0.1);
        assert!(
            left < -120.0,
            "{fx_type}: {left:.1} dB off dry long after the release, {wavs}"
        );
    }
}
