use super::audio_restart_persistence::{back, enter_edit, press, turn};
use super::in_place_menu_actions::confirm_key;
use super::in_place_menu_rows::{assert_matches_fresh_build, runner_with_instrument};
use super::*;

const LFO_VOLUME_TARGET: &str = "linkLfos.0.target.instruments.0.mixer.volume";

#[test]
fn lfo_target_pickers_gain_and_lose_the_instruments_group_like_a_fresh_build() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    confirm_key(&mut runner, "system.clearAll");
    assert!(runner.instruments.iter().all(|slot| slot.kind == "none"));
    assert!(runner.menu.item_for_key(LFO_VOLUME_TARGET).is_none());
    let rebuilds = runner.menu.rebuild_count;

    for (delta, kind) in [(1, "synth"), (-1, "none")] {
        enter_edit(&mut runner, "instruments.0.type");
        turn(&mut runner, delta);
        press(&mut runner);
        assert_eq!(runner.instruments[0].kind, kind);
        assert_eq!(
            runner.menu.item_for_key(LFO_VOLUME_TARGET).is_some(),
            kind == "synth"
        );
        assert_matches_fresh_build(&runner, rebuilds);
    }
}

#[test]
fn reset_keeps_the_focus_on_the_emptied_instrument() {
    let mut runner = runner_with_instrument("fm");
    let rebuilds = runner.menu.rebuild_count;

    confirm_key(&mut runner, "instruments.0.clone");
    assert_eq!(runner.menu.current_key(), Some("instruments.0.clone"));
    confirm_key(&mut runner, "instruments.0.reset");
    assert_eq!(runner.instruments[0].kind, "none");
    assert_eq!(runner.menu.current_key(), Some("instruments.0.type"));
    assert_matches_fresh_build(&runner, rebuilds);
}

fn rename(runner: &mut NativeRunner, key: &str) {
    let rebuilds = runner.menu.rebuild_count;
    enter_edit(runner, key);
    turn(runner, 2);
    runner.make_deferred_menu_apply_due_for_test();
    runner.flush_deferred_menu_apply().unwrap();
    assert_eq!(runner.menu.current_key(), Some(key));
    assert!(runner.menu.state.editing);
    assert_matches_fresh_build(runner, rebuilds);
    back(runner);
}

#[test]
fn renames_update_every_row_that_shows_the_name() {
    let mut runner = runner_with_instrument("sampler");

    rename(&mut runner, "instruments.0.name");
    assert!(runner.instruments[0].name.ends_with('B'));
    assert!(!runner.instruments[0].auto_name);

    rename(&mut runner, "mixer.buses.0.name");
    assert!(runner.fx_buses[0].name.ends_with('B'));
    assert!(!runner.fx_buses[0].auto_name);

    rename(&mut runner, "layers.0.name");
    assert!(runner.layer_names[0].ends_with('B'));
    assert!(!runner.layer_auto_names[0]);

    let rebuilds = runner.menu.rebuild_count;
    enter_edit(&mut runner, "layers.0.autoName");
    turn(&mut runner, 1);
    press(&mut runner);
    assert!(runner.layer_auto_names[0]);
    assert!(!runner.layer_names[0].ends_with('B'));
    assert_matches_fresh_build(&runner, rebuilds);
}

#[test]
fn midi_channel_detents_update_the_instrument_label() {
    let mut runner = runner_with_instrument("midi");
    let rebuilds = runner.menu.rebuild_count;

    enter_edit(&mut runner, "instruments.0.midi.channel");
    for _ in 0..2 {
        turn(&mut runner, 1);
        assert_matches_fresh_build(&runner, rebuilds);
    }
    press(&mut runner);
    assert_eq!(runner.instruments[0].midi_channel, 3);
    assert_matches_fresh_build(&runner, rebuilds);
}
