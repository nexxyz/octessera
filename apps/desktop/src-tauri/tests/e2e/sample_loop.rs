//! A Sampler slot's Loop set from the menu and heard on a held Keys tile: with Loop `forward`
//! the held hit keeps sounding long past the sample's own length and stops at the release; with
//! Loop `off`, the default, the same held hit ends with the sample.
use crate::support::{
    assert_clean_audio, bar_seconds, builder, keep_audio, riff_form, session, Timeline,
};
use octessera_desktop_lib::headless::{HeadlessSession, PlayedScenario};
use playback_runtime::scenario::{grid, BAR};
use std::path::{Path, PathBuf};

const SAMPLER_SLOT: u8 = 1;
/// The tile row the scenario patch stamps with sample slot 2, the short clap.
const TILE: (usize, usize) = (3, 1);
const PRESS_PULSE: u64 = BAR;
const RELEASE_PULSE: u64 = 2 * BAR;

/// Length in seconds of a PCM or float WAV file, from its `fmt ` and `data` chunks.
fn wav_seconds(path: &Path) -> f64 {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path:?}: {error}"));
    let chunks = riff_form(&bytes, b"WAVE");
    let chunk = |name: &[u8; 4]| {
        chunks
            .iter()
            .find(|(id, _)| id == name)
            .map(|(_, body)| *body)
            .unwrap_or_else(|| panic!("{path:?}: no {name:?} chunk"))
    };
    let fmt = chunk(b"fmt ");
    let rate = u32::from_le_bytes(fmt[4..8].try_into().unwrap());
    let align = u16::from_le_bytes(fmt[12..14].try_into().unwrap());
    chunk(b"data").len() as f64 / f64::from(align) / f64::from(rate)
}

struct Take {
    session: HeadlessSession,
    played: PlayedScenario,
    wav: PathBuf,
    sample_seconds: f64,
    /// Bar position of the hit's first frame.
    hit_bar: f64,
}

impl Take {
    /// RMS in dBFS between two times, in seconds after the hit.
    fn rms_db(&self, from: f64, to: f64) -> f64 {
        let bar = bar_seconds(&self.session);
        let timeline = Timeline {
            session: &self.session,
            played: self.played.clone(),
        };
        timeline.rms_db(self.hit_bar + from / bar, self.hit_bar + to / bar)
    }
}

/// The scenario patch with I2 in Note Mode hold, L2 on Keys, the clap slot's Loop at `loop_mode`
/// and its tile held from `PRESS_PULSE` to `RELEASE_PULSE`, the other layers muted.
fn play(loop_mode: &str) -> Take {
    let mut take = builder();
    take.open(&["Build", "L2:", "Beh.", "[Human]", "keys"]);
    take.open(&["Link", "L2:", "Scanning"]);
    take.set_value("Scan Mode", "layers.1.link.scanMode", "none");
    take.open(&["Link", "L2:", "Events"]);
    take.set_value("Event Triggers", "layers.1.link.eventEnabled", "true");
    take.open(&["Shape", "Instruments", "I2:"]);
    take.set_value("Note Mode", "instruments.1.noteBehavior", "hold");
    take.enter("Sampler");
    take.set_value("Sample Slot", "instruments.1.sample.selectedSlot", "2");
    assert_eq!(
        take.runner()
            .test_menu_value("instruments.1.sample.slots.1.loop")
            .as_deref(),
        Some("off"),
        "the clap slot's Loop starts off"
    );
    if loop_mode != "off" {
        take.set_value("Loop", "instruments.1.sample.slots.1.loop", loop_mode);
    }
    take.checkpoint("loop set");
    for layer in [0, 2, 3] {
        take.toggle_mute_now(layer);
    }
    take.select_layer_now(1);
    take.at(PRESS_PULSE, true, grid(TILE.0, TILE.1, true));
    take.at(RELEASE_PULSE, true, grid(TILE.0, TILE.1, false));

    let name = format!("sample-loop-{loop_mode}");
    let mut session = session(&name);
    let played = session.play_scenario(&take.build(), 0.5, 5.0).unwrap();
    let wav = keep_audio(&session, &name);
    assert_clean_audio(&session, &name);
    let slot = &session.config_payload()["runtimeConfig"]["instruments"][1]["sample"]["slots"][1];
    assert_eq!(slot["loop"], loop_mode, "{name}: saved Loop");
    let path = slot["path"].as_str().expect("the clap slot has a sample");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let hit_frame = session
        .events()
        .iter()
        .find(|event| event.slot == SAMPLER_SLOT && event.frame >= played.start_frame)
        .unwrap_or_else(|| panic!("{name}: the held tile never played, audio in {wav:?}"))
        .frame;
    let frames_per_bar = bar_seconds(&session) * f64::from(session.sample_rate());
    Take {
        sample_seconds: wav_seconds(&repo.join(path)),
        hit_bar: (hit_frame - played.start_frame) as f64 / frames_per_bar,
        session,
        played,
        wav,
    }
}

#[test]
fn a_held_sampler_hit_loops_past_the_sample_only_with_loop_on() {
    let looped = play("forward");
    let once = play("off");
    let wavs = format!("audio in {:?} and {:?}", looped.wav, once.wav);
    let len = looped.sample_seconds;
    let held = bar_seconds(&looped.session) * (RELEASE_PULSE - PRESS_PULSE) as f64 / BAR as f64;
    assert!(held > 20.0 * len, "hold {held} s against a {len} s sample");

    // Windows two sample lengths long hold at least one whole loop pass wherever they start.
    let first = looped.rms_db(0.0, 2.0 * len);
    let mut from = len;
    while from + 2.0 * len < held - 0.02 {
        let level = looped.rms_db(from, from + 2.0 * len);
        assert!(
            level > first - 6.0,
            "looped: {level:.1} dB at {from:.2} s into the hold, first pass {first:.1} dB, {wavs}"
        );
        from += len;
    }
    let after = looped.rms_db(held + 0.05, held + 0.5);
    assert!(
        after < -100.0,
        "looped: {after:.1} dB after the release, {wavs}"
    );

    let first_once = once.rms_db(0.0, len);
    assert!(
        first_once > -40.0,
        "Loop off: the hit plays at {first_once:.1} dB, {wavs}"
    );
    let tail = once.rms_db(len + 0.02, held);
    assert!(
        tail < -100.0,
        "Loop off: {tail:.1} dB after the sample ended, {wavs}"
    );
}
