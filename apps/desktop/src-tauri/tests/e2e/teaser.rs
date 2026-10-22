//! The teaser score (picture-locked timing V40): typed intro on the Play Drums page with an
//! echo chain, a plucked glitter dissolve, then the layer build-up and a held freeze.
use crate::support::builder;
use playback_runtime::scenario::{button, grid, press_main, Scenario, ScenarioBuilder};

/// Seconds after B1 of bar `n` (1-based) in the build-up, after the intro's 1.5 s pause.
pub fn bar(n: u32) -> f64 {
    9.5 + 2.0 * f64::from(n - 5)
}

/// Typed lines: (start, letters with spaces, letter spacing, stamp row).
const LINES: [(f64, &str, f64, usize); 4] = [
    (0.0, "Eight rows", 0.1, 7),
    (2.0, "eight columns", 0.077, 6),
    (4.0, "eight layers", 0.083, 5),
    (6.0, "one device", 0.07, 4),
];
/// The "..." after the first three lines, then the final "." of "one device.".
const DOTS: [f64; 9] = [
    1.100, 1.167, 1.233, 3.083, 3.150, 3.200, 5.067, 5.083, 5.133,
];
const PERIOD: f64 = 6.767;
const DOT_ROW: usize = 3;
const PERIOD_ROW: usize = 2;
pub const ECHO_KILL: f64 = 6.85;
pub const GLITTER: f64 = 8.5;
pub const FREEZE: f64 = 29.5;
pub const PAUSE: f64 = 37.4;
pub const RELEASE: f64 = 39.3;
pub const END: f64 = 39.9;
const FREEZE_CELL: (usize, usize) = (2, 6);
const CLAP_HATS: [(usize, usize); 6] = [(2, 1), (6, 1), (1, 2), (3, 2), (5, 2), (7, 2)];

/// Letter times for one typed line: steady with a small, repeatable unevenness; no hit on
/// spaces.
pub fn letter_times() -> Vec<(f64, usize, usize)> {
    let mut hits = Vec::new();
    for (start, text, spacing, row) in LINES {
        for (index, letter) in text.chars().enumerate() {
            if letter == ' ' {
                continue;
            }
            let jitter = [0.0, 0.012, -0.008, 0.018, -0.004, 0.009][index % 6];
            hits.push((start + index as f64 * spacing + jitter, index % 8, row));
        }
    }
    hits
}

/// A drum voice, its index, and the Edit rows to set: (row label, field, value).
type TablaVoice = (&'static str, usize, [(&'static str, &'static str, i32); 5]);

fn typing_kit(take: &mut ScenarioBuilder) {
    take.open(&["Shape", "Instruments", "I5:"]);
    take.set_value("Type", "instruments.4.type", "drum");
    take.checkpoint("i5 drum");
    let voices: [TablaVoice; 3] = [
        (
            "V6",
            5,
            [
                ("Tone", "tonePct", 85),
                ("Decay", "decayMs", 180),
                ("Sweep st", "sweepSemis", 12),
                ("Sweep ms", "sweepMs", 6),
                ("Noise %", "noiseMixPct", 12),
            ],
        ),
        (
            "V8",
            7,
            [
                ("Tone", "tonePct", 80),
                ("Decay", "decayMs", 120),
                ("Sweep st", "sweepSemis", 5),
                ("Sweep ms", "sweepMs", 5),
                ("Noise %", "noiseMixPct", 20),
            ],
        ),
        (
            "V5",
            4,
            [
                ("Tone", "tonePct", 60),
                ("Decay", "decayMs", 350),
                ("Sweep st", "sweepSemis", 7),
                ("Sweep ms", "sweepMs", 80),
                ("Noise %", "noiseMixPct", 8),
            ],
        ),
    ];
    for (voice, index, edits) in voices {
        take.open(&["Shape", "Instruments", "I5:", "Drum"]);
        take.set_value("Voice", "instruments.4.drum.voice", voice);
        take.enter("Edit");
        for (row, field, target) in edits {
            take.set_number(
                row,
                &format!("instruments.4.drum.voices.{index}.{field}"),
                target,
            );
        }
    }
    let rows: [(&str, usize, i32); 6] = [
        ("V6", 7, 0),
        ("V6", 6, 2),
        ("V6", 5, 5),
        ("V6", 4, 7),
        ("V8", DOT_ROW, 12),
        ("V5", PERIOD_ROW, 0),
    ];
    take.open(&["Shape", "Instruments", "I5:", "Drum"]);
    for (voice, row, tune) in rows {
        take.set_value("Voice", "instruments.4.drum.voice", voice);
        take.set_number("Cell Tune", "instruments.4.cell.tuneSemis", tune);
        take.enter("Assign");
        take.with_mods_now(
            &["button_shift"],
            &[grid(0, row, true), grid(0, row, false)],
        );
        take.input(button("button_a", true));
        take.input(button("button_a", false));
    }
    take.checkpoint("typing kit");
    take.open(&["Shape", "Instruments", "I5:", "Mixer"]);
    take.set_value("Route", "instruments.4.mixer.route", "fx_bus_3");
}

/// Bus 3: triplet echo, auto-pan and a light bitcrusher; Aux 2 is bound to the echo's Mix.
fn echo_bus(take: &mut ScenarioBuilder) {
    let slot = |n: u32| format!("mixer.buses.2.slot{n}");
    take.open(&["Shape", "FX Buses", "B3:", "Slot 1"]);
    take.set_value("Type", &format!("{}.type", slot(1)), "delay");
    take.set_value("Time Mode", &format!("{}.params.timeMode", slot(1)), "note");
    take.set_value(
        "Time Note",
        &format!("{}.params.timeNote", slot(1)),
        "1/16T",
    );
    take.set_number("Feedback", &format!("{}.params.feedback", slot(1)), 45);
    take.set_number("Spread %", &format!("{}.params.spreadPct", slot(1)), 60);
    take.set_number("Mix %", &format!("{}.params.mixPct", slot(1)), 35);
    take.select("Mix %");
    take.input(press_main());
    take.with_mods_now(&["button_fn"], &[json_press("aux2")]);
    take.input(press_main());
    take.open(&["Shape", "FX Buses", "B3:", "Slot 2"]);
    take.set_value("Type", &format!("{}.type", slot(2)), "auto_pan");
    take.set_number("Rate Hz", &format!("{}.params.rateHz", slot(2)), 300);
    take.set_number("Depth %", &format!("{}.params.depthPct", slot(2)), 60);
    take.open(&["Shape", "FX Buses", "B3:", "Slot 3"]);
    take.set_value("Type", &format!("{}.type", slot(3)), "bitcrusher");
    take.set_number("Bits", &format!("{}.params.bits", slot(3)), 8);
    take.set_number("Mix %", &format!("{}.params.mixPct", slot(3)), 25);
    take.checkpoint("echo bus");
}

fn json_press(id: &str) -> serde_json::Value {
    serde_json::json!({ "type": "encoder_press", "id": id })
}

fn glitter_layer(take: &mut ScenarioBuilder) {
    take.open(&["Shape", "Instruments", "I6:"]);
    take.set_value("Type", "instruments.5.type", "pluck");
    take.open(&[
        "Shape",
        "Instruments",
        "I6:",
        "Plucked",
        "Preset",
        "Load",
        "Steel",
    ]);
    take.confirm();
    take.open(&["Shape", "Instruments", "I6:", "Plucked", "String"]);
    take.set_number("Decay", "instruments.5.pluck.decayMs", 900);
    take.open(&["Build", "L6:", "Behavior", "[Cellular]", "twinkle"]);
    take.open(&["Build", "L6:"]);
    take.set_value("Step Rate", "algorithmStep", "1/32");
    let config = "layers.5.build.behaviorConfig";
    take.set_number("Density", &format!("{config}.density"), 5);
    take.set_number("Birth Chance", &format!("{config}.birthChancePct"), 100);
    take.set_number("Fade Chance", &format!("{config}.fadeChancePct"), 100);
    take.set_number("Star Life", &format!("{config}.starLife"), 1);
    take.set_number("Cluster Bias", &format!("{config}.clusterBiasPct"), 0);
    take.open(&["Link", "L6:", "Events"]);
    if take
        .runner()
        .test_menu_value("layers.5.link.eventEnabled")
        .as_deref()
        != Some("true")
    {
        take.set_value("Event Triggers", "layers.5.link.eventEnabled", "true");
    }
    take.open(&["Link", "L6:", "X Axis", "Pitch Steps"]);
    take.set_number("Steps", "layers.5.link.x.pitch.steps", 1);
    take.open(&["Link", "L6:", "Y Axis", "Pitch Steps"]);
    take.set_number("Steps", "layers.5.link.y.pitch.steps", 8);
    take.open(&["Link", "L6:", "Note Mapping"]);
    take.set_number("Low Note", "layers.5.link.pitch.lowestNote", 48);
    take.set_number("High Note", "layers.5.link.pitch.highestNote", 96);
    take.set_value("Out of Range", "layers.5.link.pitch.outOfRange", "wrap");
    take.checkpoint("glitter");
}

pub fn teaser_v40(record: bool) -> Scenario {
    let mut take = builder();
    typing_kit(&mut take);
    echo_bus(&mut take);
    glitter_layer(&mut take);
    take.select_layer_now(1);
    for (x, y) in CLAP_HATS {
        take.tap_now(x, y);
    }
    for layer in 0..4 {
        take.toggle_mute_now(layer);
    }
    take.back_to_root();
    if record {
        take.open(&["System", "Recording", "Audio+Video"]);
        take.back_to_root();
    }
    take.select_layer_now(0);
    take.with_mods_now(&["button_fn"], &[grid(7, 6, true), grid(7, 6, false)]);
    take.checkpoint("drums page");
    take.without_play_at_b1();

    for (time, x, row) in letter_times() {
        take.at_seconds(time, grid(x, row, true));
        take.at_seconds(time + 0.04, grid(x, row, false));
    }
    for (index, time) in DOTS.iter().enumerate() {
        take.at_seconds(*time, grid(index % 3, DOT_ROW, true));
        take.at_seconds(time + 0.03, grid(index % 3, DOT_ROW, false));
    }
    take.at_seconds(PERIOD, grid(0, PERIOD_ROW, true));
    take.at_seconds(PERIOD + 0.04, grid(0, PERIOD_ROW, false));
    take.at_seconds(ECHO_KILL, aux_turn("aux2", -40));
    // Twinkle's first star is born on the step after Play, so Play goes one 1/32 step early.
    take.at_seconds(GLITTER - 0.0625, button("button_s", true));
    take.at_seconds(GLITTER - 0.0125, button("button_s", false));

    take.gesture_at_seconds(
        &["button_fn"],
        bar(5) - 0.3,
        &[button("button_s", true), button("button_s", false)],
    );
    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        bar(5) - 0.08,
        &[
            grid(0, 5, true),
            grid(0, 5, false),
            grid(0, 0, true),
            grid(0, 0, false),
        ],
    );
    take.at_seconds(bar(5), button("button_s", true));
    take.at_seconds(bar(5) + 0.02, button("button_s", false));
    take.gesture_at_seconds(
        &["button_fn"],
        bar(5) + 0.2,
        &[grid(0, 0, true), grid(0, 0, false)],
    );
    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        bar(7) - 0.01,
        &[
            grid(0, 1, true),
            grid(0, 1, false),
            grid(0, 3, true),
            grid(0, 3, false),
        ],
    );
    take.gesture_at_seconds(
        &["button_fn"],
        bar(9) - 0.3,
        &[grid(0, 1, true), grid(0, 1, false)],
    );
    for (x, y) in CLAP_HATS {
        take.at_seconds(bar(9) - 0.01, grid(x, y, true));
        take.at_seconds(bar(9) + 0.05, grid(x, y, false));
    }
    take.gesture_at_seconds(
        &["button_fn"],
        bar(9) + 0.3,
        &[grid(0, 0, true), grid(0, 0, false)],
    );
    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        bar(11) - 0.01,
        &[grid(0, 2, true), grid(0, 2, false)],
    );
    take.gesture_at_seconds(
        &["button_fn"],
        bar(14) + 1.5,
        &[grid(7, 2, true), grid(7, 2, false)],
    );
    take.at_seconds(FREEZE + 0.003, grid(FREEZE_CELL.0, FREEZE_CELL.1, true));
    take.at_seconds(PAUSE, button("button_s", true));
    take.at_seconds(PAUSE + 0.05, button("button_s", false));
    take.at_seconds(RELEASE, grid(FREEZE_CELL.0, FREEZE_CELL.1, false));
    take.build()
}

fn aux_turn(id: &str, delta: i32) -> serde_json::Value {
    serde_json::json!({ "type": "encoder_turn", "id": id, "delta": delta })
}
