//! Every behavior chosen from the Build menu on L1 (default config, the default patch's I1 Synth)
//! keeps sounding through the device path as often as its default-init cadence promises, and
//! the input-only ones stay silent. One test per Build category, so they run side by side.
#[path = "../../../../../crates/platform-core/src/behaviors/activate_cadence_table.rs"]
mod activate_cadence_table;

use crate::support::{builder, keep_audio, session, Timeline};
use activate_cadence_table::{Expect, CADENCE};
use playback_runtime::scenario::BAR;

const BARS: u64 = 16;
const SYNTH: u8 = 0;

/// Pulses per step of a Step Rate such as `1/16` or `1/8T`.
fn step_pulses(rate: &str) -> u64 {
    let (straight, triplet) = match rate.strip_suffix('T') {
        Some(straight) => (straight, true),
        None => (rate, false),
    };
    let (numerator, denominator) = straight.split_once('/').unwrap();
    let pulses = BAR * numerator.parse::<u64>().unwrap() / denominator.parse::<u64>().unwrap();
    if triplet {
        pulses * 2 / 3
    } else {
        pulses
    }
}

/// The longest stretch of pulses without a note-on from L1, counting from Play to the end.
fn longest_gap(pulses: &[u64], end: u64) -> u64 {
    let mut last = 0;
    let mut longest = 0;
    for pulse in pulses.iter().copied().chain([end]) {
        longest = longest.max(pulse - last);
        last = pulse;
    }
    longest
}

fn check_behavior(category: &str, id: &str, label: &str) {
    let row = CADENCE
        .iter()
        .find(|row| row.id == id)
        .unwrap_or_else(|| panic!("{id} has no cadence row"));
    let mut take = builder();
    take.open(&["Build", "L1", "Beh.", &format!("[{category}]"), label]);
    take.checkpoint(id);
    take.back_to_root();
    let name = format!("behavior-{id}");
    let mut session = session(&name);
    let played = session
        .play_scenario(&take.build(), 0.5, BARS as f64 * 2.0)
        .unwrap();
    let layer = &session.config_payload()["runtimeConfig"]["layers"][0]["build"];
    assert_eq!(layer["behaviorId"], id, "the Build menu picked {label}");
    let step_rate = layer["stepRate"].as_str().map(str::to_string);
    let timeline = Timeline {
        session: &session,
        played,
    };
    let start = timeline.played.start_pulse;
    let mut pulses: Vec<u64> = timeline
        .events(0.0, BARS as f64)
        .iter()
        .filter(|event| event.slot == SYNTH)
        .map(|event| event.pulse - start)
        .collect();
    pulses.dedup();
    match row.expect {
        Expect::Silent => assert!(
            pulses.is_empty(),
            "{id} played at pulses {pulses:?} without input ({})",
            row.why
        ),
        Expect::Within(window) => {
            let step = step_pulses(step_rate.as_deref().unwrap());
            let allowed = (window as u64 + 1) * step;
            if allowed <= BARS * BAR {
                let gap = longest_gap(&pulses, BARS * BAR);
                assert!(
                    gap <= allowed,
                    "{id}: {gap} pulses without a note, expected at most {allowed} ({}), audio in {:?}",
                    row.why,
                    keep_audio(&session, &name)
                );
            }
        }
    }
}

fn check_category(category_id: &str) {
    let category = platform_core::behavior_categories()
        .iter()
        .find(|category| category.id == category_id)
        .unwrap();
    for id in category.behavior_ids {
        let entry = platform_core::behavior_catalog()
            .iter()
            .find(|entry| entry.id == *id)
            .unwrap();
        check_behavior(category.label, id, entry.label);
    }
}

#[test]
fn human_behaviors_follow_their_cadence() {
    check_category("play");
}

#[test]
fn rhythm_behaviors_follow_their_cadence() {
    check_category("rhythm");
}

#[test]
fn musical_behaviors_follow_their_cadence() {
    check_category("musical");
}

#[test]
fn cellular_behaviors_follow_their_cadence() {
    check_category("cellular");
}

#[test]
fn fields_behaviors_follow_their_cadence() {
    check_category("fields");
}

#[test]
fn geometry_behaviors_follow_their_cadence() {
    check_category("geometry");
}

#[test]
fn growth_behaviors_follow_their_cadence() {
    check_category("growth");
}

#[test]
fn motion_behaviors_follow_their_cadence() {
    check_category("motion");
}

#[test]
fn games_behaviors_follow_their_cadence() {
    check_category("games");
}

#[test]
fn every_behavior_sits_in_one_build_category() {
    let listed: usize = platform_core::behavior_categories()
        .iter()
        .map(|category| category.behavior_ids.len())
        .sum();
    assert_eq!(listed, CADENCE.len());
}
