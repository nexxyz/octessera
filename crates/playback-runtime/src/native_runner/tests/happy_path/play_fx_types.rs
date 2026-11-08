use super::*;

/// The new types after `none` and the four original types, each at a different insertion point.
const NEW_TYPES: [(&str, &str); 7] = [
    ("tape_stop", "master"),
    ("reverse", "fx_bus_1"),
    ("trance_gate", "instrument_1"),
    ("dub_throw", "master"),
    ("wash", "fx_bus_2"),
    ("hp_riser", "instrument_2"),
    ("crush", "master"),
];

fn momentary_starts(messages: &[RunnerMessage]) -> Vec<(String, BTreeMap<String, Value>)> {
    messages
        .iter()
        .filter_map(|message| match message {
            RunnerMessage::PlatformEffects { effects } => Some(effects),
            _ => None,
        })
        .flatten()
        .filter_map(|effect| match effect {
            RuntimePlatformEffect::AudioCommand {
                command:
                    RuntimeAudioCommand::MomentaryFxStart {
                        fx_type, params, ..
                    },
            } => Some((fx_type.clone(), params.clone())),
            _ => None,
        })
        .collect()
}

fn toast(runner: &NativeRunner) -> String {
    runner
        .display
        .toast
        .as_ref()
        .map(|toast| toast.message.clone())
        .unwrap_or_default()
}

/// Turns the enum row `key` straight to the option `value`.
fn turn_until(runner: &mut NativeRunner, key: &str, value: &str) {
    let Some(crate::native_menu::NativeMenuValue::Enum { options, selected }) =
        runner.menu.item_for_key(key).map(|item| item.value.clone())
    else {
        panic!("{key} is not an enum row");
    };
    let wanted = options
        .iter()
        .position(|option| option == value)
        .unwrap_or_else(|| panic!("{key} has no option {value}"));
    if wanted != selected {
        assert_snapshot(edit_key_by_turns(
            runner,
            key,
            wanted as i32 - selected as i32,
        ));
    }
    assert_eq!(runner.menu.value_for_key(key).as_deref(), Some(value));
}

/// Picks `fx_type` and `target` on the FX page and maps the current values to tile (x, 6).
fn map_type_to_tile(runner: &mut NativeRunner, fx_type: &str, target: &str, x: usize) {
    assert!(runner.menu.focus_item_key("play.page.fx"));
    let _ = press_main(runner);
    assert_eq!(runner.active_play_mode, ActivePlayMode::Page(PlayMode::Fx));
    turn_until(runner, "play.fx.type", fx_type);
    turn_until(runner, "play.fx.target", target);
    assert!(runner.menu.focus_item_key("play.fx.map"));
    let _ = press_main(runner);
    let shown =
        crate::native_menu::format_values::format_display_value(Some("play.fx.type"), fx_type);
    assert_eq!(toast(runner), format!("FX {shown}: press tiles to assign"));
    assert_snapshot(grid_press(runner, x, 6));
    let _ = grid_release(runner, x, 6);
    assert_eq!(toast(runner), format!("FX {shown}"));
}

#[test]
pub(crate) fn every_new_play_fx_type_maps_to_a_tile_and_plays_while_held() {
    let mut runner = runner();
    for (index, (fx_type, target)) in NEW_TYPES.into_iter().enumerate() {
        map_type_to_tile(&mut runner, fx_type, target, index);
        let stored = runner
            .play_fx_assignments
            .iter()
            .find(|assignment| assignment.x == index && assignment.y == 6)
            .expect("mapped tile");
        assert_eq!(stored.config["fxType"], fx_type);
        assert_eq!(stored.config["targetKey"], target);

        let starts = momentary_starts(&grid_press(&mut runner, index, 6));
        assert_eq!(starts.len(), 1, "{fx_type} starts once");
        let (started, params) = &starts[0];
        assert_eq!(started, fx_type);
        match fx_type {
            "trance_gate" => {
                assert_eq!(params.get("ratePulses"), Some(&json!(6)), "1/16 in pulses");
                assert_eq!(params.get("tempoBpm"), Some(&json!(runner.transport.bpm)));
                assert!(!params.contains_key("rateNote"));
            }
            "dub_throw" => {
                assert_eq!(params.get("timePulses"), Some(&json!(24)), "1/4 in pulses");
                assert!(params.contains_key("tempoBpm"));
            }
            _ => assert!(
                !params.contains_key("tempoBpm"),
                "{fx_type} is not tempo-synced"
            ),
        }
        assert!(contains_momentary_stop(&grid_release(
            &mut runner,
            index,
            6
        )));
        assert!(runner.active_play_fx.is_empty());
    }
}

#[test]
pub(crate) fn new_play_fx_types_respect_the_concurrency_limits() {
    let mut runner = runner();
    for (index, (fx_type, target)) in NEW_TYPES.into_iter().take(3).enumerate() {
        map_type_to_tile(&mut runner, fx_type, target, index);
    }
    map_type_to_tile(&mut runner, "tape_stop", "fx_bus_1", 3);

    assert_eq!(momentary_starts(&grid_press(&mut runner, 0, 6)).len(), 1);
    assert!(momentary_starts(&grid_press(&mut runner, 3, 6)).is_empty());
    assert_eq!(toast(&runner), "FX tape stop already on");
    assert_eq!(momentary_starts(&grid_press(&mut runner, 1, 6)).len(), 1);
    assert!(momentary_starts(&grid_press(&mut runner, 2, 6)).is_empty());
    assert_eq!(toast(&runner), "FX limit: 2 at once");
    assert_eq!(runner.active_play_fx.len(), 2);

    let _ = grid_release(&mut runner, 0, 6);
    assert_eq!(momentary_starts(&grid_press(&mut runner, 2, 6)).len(), 1);
}

#[test]
pub(crate) fn a_tempo_change_retimes_a_held_trance_gate() {
    let mut runner = runner();
    map_type_to_tile(&mut runner, "trance_gate", "master", 0);
    let _ = grid_press(&mut runner, 0, 6);
    assert!(runner.menu.focus_item_key("transport.bpm"));
    let messages = edit_key_by_turns(&mut runner, "transport.bpm", 10);
    let bpm = runner.transport.bpm;
    let retimed = messages.iter().any(|message| {
        matches!(
            message,
            RunnerMessage::AudioCommands { commands }
                if commands.iter().any(|command| matches!(
                    command,
                    RuntimeAudioCommand::MomentaryFxUpdate { id, params, .. }
                        if id == "momentary-fx:0:6"
                            && params.get("tempoBpm") == Some(&json!(bpm))
                            && params.get("ratePulses") == Some(&json!(6))
                ))
        )
    });
    assert!(retimed, "held gate follows the new tempo {bpm}");
}

#[test]
pub(crate) fn every_play_fx_type_has_its_own_led_color() {
    let colors = crate::native_runner::play_fx_config::play_fx_types()
        .filter(|fx_type| *fx_type != "none")
        .map(|fx_type| {
            let color = crate::native_runner::play_fx_presentation::play_fx_color(fx_type);
            assert_ne!(
                color,
                crate::native_runner::led_color::LedColor::SYSTEM,
                "{fx_type}"
            );
            format!("{color:?}")
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(colors.len(), 11);
}

#[test]
pub(crate) fn the_shipped_patch_maps_four_distinct_stamps_of_every_play_fx_type() {
    let base: Value =
        serde_json::from_str(include_str!("../../../../../../config/defaults/base.json")).unwrap();
    let assignments = base["runtimeConfig"]["playFx"]["assignments"]
        .as_array()
        .unwrap();
    let tiles = assignments
        .iter()
        .map(|assignment| (assignment["x"].as_u64(), assignment["y"].as_u64()))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(tiles.len(), assignments.len(), "one stamp per tile");
    for fx_type in crate::native_runner::play_fx_config::play_fx_types().skip(1) {
        let stamps = assignments
            .iter()
            .map(|assignment| &assignment["config"])
            .filter(|config| config["fxType"] == fx_type)
            .collect::<Vec<_>>();
        assert_eq!(stamps.len(), 4, "{fx_type} has one 2x2 block");
        let distinct = stamps
            .iter()
            .map(|config| config["params"].to_string())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(distinct.len(), 4, "{fx_type} stamps differ");
        for config in stamps {
            assert_eq!(
                &crate::native_runner::play_fx_config::sanitize_play_fx_config(config),
                config,
                "{fx_type} stamp is in range and complete"
            );
        }
    }
}
