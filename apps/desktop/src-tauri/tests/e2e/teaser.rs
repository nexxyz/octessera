//! The teaser score (picture-locked timing V51): typed intro on the Play Drums page with an
//! echo chain, a plucked glitter dissolve, then the layer build-up and a held freeze.
use crate::support::builder;
use playback_runtime::scenario::{button, grid, press_main, Scenario, ScenarioBuilder};

/// Seconds after B1 of bar `n` (1-based) in the build-up, which starts after the glitter has
/// rung out.
pub fn bar(n: u32) -> f64 {
    BUILD_START + 2.0 * f64::from(n - 5)
}

/// Typed lines: (keystrokes, their times after B1, stamp row). Commas sound on `COMMA_ROW`.
const LINES: [(&str, &[f64], usize); 5] = [
    (
        "64cells,",
        &[0.000, 0.081, 0.210, 0.273, 0.350, 0.414, 0.491, 0.566],
        7,
    ),
    (
        "8layers,",
        &[2.000, 2.150, 2.227, 2.288, 2.364, 2.432, 2.514, 2.577],
        6,
    ),
    (
        "40+algorithms,",
        &[
            4.000, 4.081, 4.153, 4.301, 4.364, 4.424, 4.484, 4.551, 4.611, 4.673, 4.742, 4.809,
            4.880, 4.947,
        ],
        5,
    ),
    (
        "endlesspossibilities",
        &[
            6.000, 6.073, 6.152, 6.211, 6.287, 6.354, 6.418, 6.549, 6.624, 6.706, 6.772, 6.853,
            6.924, 7.003, 7.064, 7.123, 7.191, 7.250, 7.315, 7.374,
        ],
        4,
    ),
    (
        "onedevice",
        &[
            9.000, 9.060, 9.131, 9.282, 9.347, 9.426, 9.502, 9.563, 9.631,
        ],
        7,
    ),
];
const COMMA_ROW: usize = 3;
/// The echo is cut after "endless possibilities", so the break and "one device" stay dry.
pub const ECHO_KILL: f64 = 7.45;
pub const GLITTER: f64 = 11.5;
/// The glitter layer is muted here and its notes, delay and reverb ring out.
pub const GLITTER_END: f64 = 12.5;
const RING_OUT: f64 = 1.5;
pub const BUILD_START: f64 = GLITTER_END + RING_OUT;
pub const FREEZE: f64 = BUILD_START + 20.0;
pub const PAUSE: f64 = FREEZE + 7.9;
pub const RELEASE: f64 = FREEZE + 9.8;
pub const END: f64 = FREEZE + 10.4;
const FREEZE_CELL: (usize, usize) = (2, 6);
const KICKS: [(usize, usize); 4] = [(0, 0), (2, 0), (4, 0), (6, 0)];
const CLAP_HATS: [(usize, usize); 6] = [(2, 1), (6, 1), (1, 2), (3, 2), (5, 2), (7, 2)];

/// Every keystroke of the intro: (time after B1, column, row).
pub fn keystrokes() -> Vec<(f64, usize, usize)> {
    let mut hits = Vec::new();
    for (keys, times, row) in LINES {
        assert_eq!(keys.chars().count(), times.len(), "{keys}");
        for (index, (key, time)) in keys.chars().zip(times).enumerate() {
            let row = if key == ',' { COMMA_ROW } else { row };
            hits.push((*time, index % 8, row));
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
    let voices: [TablaVoice; 2] = [
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
    let rows: [(&str, usize, i32); 5] = [
        ("V6", 7, 0),
        ("V6", 6, 2),
        ("V6", 5, 5),
        ("V6", 4, 7),
        ("V8", COMMA_ROW, 12),
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

/// L2 on two pages: page 1 the kick alone, page 2 the full beat. Aux 3 turns the page.
fn beat_pages(take: &mut ScenarioBuilder) {
    let config = "layers.1.build.behaviorConfig";
    take.open(&["Build", "L2:"]);
    take.set_number("Pages X", &format!("{config}.pagesX"), 2);
    take.set_number("Page X", &format!("{config}.pageX"), 2);
    take.select_layer_now(1);
    for (x, y) in KICKS.into_iter().chain(CLAP_HATS) {
        take.tap_now(x, y);
    }
    take.open(&["Build", "L2:"]);
    take.set_number("Page X", &format!("{config}.pageX"), 1);
    take.select("Page X");
    take.input(press_main());
    take.with_mods_now(&["button_fn"], &[json_press("aux3")]);
    take.input(press_main());
    take.select_layer_now(1);
    for (x, y) in CLAP_HATS {
        take.tap_now(x, y);
    }
    take.checkpoint("beat pages");
}

fn json_press(id: &str) -> serde_json::Value {
    serde_json::json!({ "type": "encoder_press", "id": id })
}

/// Bus 4 for the glitter: a wide 1/8-triplet delay into a long reverb, so the dissolve blooms.
fn glitter_bus(take: &mut ScenarioBuilder) {
    let slot = |n: u32| format!("mixer.buses.3.slot{n}");
    take.open(&["Shape", "FX Buses", "B4:", "Slot 1"]);
    take.checkpoint("glitter slot 1");
    take.set_value("Type", &format!("{}.type", slot(1)), "delay");
    take.set_value("Time Mode", &format!("{}.params.timeMode", slot(1)), "note");
    take.set_value("Time Note", &format!("{}.params.timeNote", slot(1)), "1/8T");
    take.set_number("Feedback", &format!("{}.params.feedback", slot(1)), 40);
    take.set_number("Spread %", &format!("{}.params.spreadPct", slot(1)), 80);
    take.set_number("Mix %", &format!("{}.params.mixPct", slot(1)), 35);
    take.checkpoint("glitter delay");
    take.open(&["Shape", "FX Buses", "B4:", "Slot 2"]);
    take.checkpoint("glitter slot 2");
    take.set_value("Type", &format!("{}.type", slot(2)), "reverb");
    take.set_number("Decay", &format!("{}.params.decay", slot(2)), 850);
    take.set_number("Damp", &format!("{}.params.damp", slot(2)), 30);
    take.set_number("Mix %", &format!("{}.params.mixPct", slot(2)), 45);
    take.checkpoint("glitter reverb");
    take.open(&["Shape", "Instruments", "I6:", "Mixer"]);
    take.set_value("Route", "instruments.5.mixer.route", "fx_bus_4");
    take.checkpoint("glitter bus");
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

pub fn teaser_v51(record: bool) -> Scenario {
    let mut take = builder();
    typing_kit(&mut take);
    echo_bus(&mut take);
    glitter_layer(&mut take);
    glitter_bus(&mut take);
    beat_pages(&mut take);
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

    for (time, x, row) in keystrokes() {
        take.at_seconds(time, grid(x, row, true));
        take.at_seconds(time + 0.04, grid(x, row, false));
    }
    take.at_seconds(ECHO_KILL, aux_turn("aux2", -40));
    // Twinkle's first star is born on the step after Play, so Play goes one 1/32 step early.
    take.at_seconds(GLITTER - 0.0625, button("button_s", true));
    take.at_seconds(GLITTER - 0.0125, button("button_s", false));

    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        GLITTER_END - 0.005,
        &[grid(0, 5, true), grid(0, 5, false)],
    );
    take.gesture_at_seconds(
        &["button_fn"],
        bar(5) - 0.3,
        &[button("button_s", true), button("button_s", false)],
    );
    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        bar(5) - 0.08,
        &[grid(0, 0, true), grid(0, 0, false)],
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
            grid(0, 0, true),
            grid(0, 0, false),
            grid(0, 1, true),
            grid(0, 1, false),
            grid(0, 3, true),
            grid(0, 3, false),
        ],
    );
    // Bar 9: Aux 3 turns L2 to its full-beat page and Life comes back, both on the downbeat.
    take.at_seconds(bar(9) - 0.2, aux_turn("aux3", 1));
    take.gesture_at_seconds(
        &["button_shift", "button_fn"],
        bar(9) - 0.01,
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
