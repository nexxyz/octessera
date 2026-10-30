use super::audio_restart_persistence::{enter_edit, press, turn};
use super::*;
use crate::native_menu::NativeMenuModel;

pub(super) fn assert_matches_fresh_build(runner: &NativeRunner, rebuilds: usize) {
    assert_eq!(runner.menu.rebuild_count, rebuilds);
    assert_eq!(
        runner.menu.root,
        NativeMenuModel::new(runner.menu_config()).root
    );
}

fn assert_detents_match_fresh_build(runner: &mut NativeRunner, key: &str, deltas: &[i32]) {
    let rebuilds = runner.menu.rebuild_count;
    enter_edit(runner, key);
    for delta in deltas {
        turn(runner, *delta);
        assert_matches_fresh_build(runner, rebuilds);
        assert_eq!(runner.menu.current_key(), Some(key));
        assert!(runner.menu.state.editing);
    }
    press(runner);
    assert_matches_fresh_build(runner, rebuilds);
}

pub(super) fn runner_with_instrument(kind: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = kind.into();
    runner.menu.rebuild(runner.menu_config());
    runner
}

fn send_sample_list(runner: &mut NativeRunner, dir: &str, entries: Vec<SampleEntry>) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot: 0,
                sample_slot: 0,
                dir: dir.into(),
                entries,
            },
        })
        .unwrap();
}

fn sample_entry(name: &str, path: &str, is_dir: bool) -> SampleEntry {
    SampleEntry {
        name: name.into(),
        path: path.into(),
        is_dir,
    }
}

#[test]
fn binding_range_detents_update_their_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.xy_x_binding = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: Some("Volume".into()),
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());

    assert_detents_match_fresh_build(&mut runner, "xy:x.rangeMin", &[10, 10, 20]);
    assert_detents_match_fresh_build(&mut runner, "xy:x.rangeMax", &[-30, -40, -20]);
    let binding = runner.xy_x_binding.as_ref().unwrap();
    assert!(binding.user_min.is_some() && binding.user_max.is_some());
}

#[test]
fn play_fx_type_detents_update_their_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_detents_match_fresh_build(&mut runner, "play.fx.type", &[1, 1, 1, 1, -4, 2]);
    assert!(runner.menu.focus_item_key("play.fx.params.releaseMs"));
}

#[test]
fn drum_voice_and_sound_detents_update_their_rows_in_place() {
    let mut runner = runner_with_instrument("drum");

    assert_detents_match_fresh_build(&mut runner, "instruments.0.drum.voice", &[1, 1, 3, -2]);
    assert_eq!(runner.drum_selected_voices[0], 3);
    let sound = runner.instruments[0].drum_config["voices"][3]["sound"].clone();
    assert_detents_match_fresh_build(
        &mut runner,
        "instruments.0.drum.voices.3.sound",
        &[1, 2, -1],
    );
    assert_ne!(
        runner.instruments[0].drum_config["voices"][3]["sound"],
        sound
    );
}

#[test]
fn sampler_slot_and_velocity_level_detents_update_their_rows_in_place() {
    let mut runner = runner_with_instrument("sampler");

    assert_detents_match_fresh_build(&mut runner, "instruments.0.sample.selectedSlot", &[1, 2]);
    assert_eq!(runner.instruments[0].selected_sample_slot, 3);
    assert_detents_match_fresh_build(
        &mut runner,
        "instruments.0.sample.velocityLevelsEnabled",
        &[1, -1, 1],
    );
    assert!(runner.instruments[0].sample_velocity_levels_enabled);
}

#[test]
fn sampler_field_detents_apply_only_their_field() {
    let mut runner = runner_with_instrument("sampler");
    let before = runner.instruments[0].sample_amp_env.clone();

    assert_detents_match_fresh_build(&mut runner, "instruments.0.sample.ampEnv.attackMs", &[3, 3]);
    assert_detents_match_fresh_build(
        &mut runner,
        "instruments.0.sample.filter.envAmountPct",
        &[-5],
    );
    assert_detents_match_fresh_build(&mut runner, "instruments.0.sample.baseVelocity", &[-7]);

    assert_ne!(
        runner.instruments[0].sample_amp_env["attackMs"],
        before["attackMs"]
    );
    assert_eq!(
        runner.instruments[0].sample_amp_env["releaseMs"],
        before["releaseMs"]
    );
    assert!(runner.instruments[0].sample_filter["envAmountPct"].as_i64() < Some(0));
    assert!(runner.instruments[0].sample_base_velocity < 100);
}

#[test]
fn link_scan_mode_detents_update_their_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_detents_match_fresh_build(&mut runner, "layers.0.link.scanMode", &[1, -1, 1]);
    assert!(runner.menu.focus_item_key("layers.0.link.scanAxis"));
}

#[test]
fn sample_browser_steps_update_only_the_browser_rows() {
    let mut runner = runner_with_instrument("sampler");
    let rebuilds = runner.menu.rebuild_count;

    assert!(runner.menu.focus_item_key("sample.choose:0:0"));
    press(&mut runner);
    assert_matches_fresh_build(&runner, rebuilds);
    assert_eq!(runner.menu.current_key(), Some("sample.up.0.0"));

    send_sample_list(
        &mut runner,
        "",
        vec![
            sample_entry("Drums", "Drums", true),
            sample_entry("pad.wav", "pad.wav", false),
        ],
    );
    assert_matches_fresh_build(&runner, rebuilds);

    assert!(runner.menu.focus_item_key("sample.enter.0.0.Drums"));
    press(&mut runner);
    assert_matches_fresh_build(&runner, rebuilds);
    send_sample_list(
        &mut runner,
        "Drums",
        vec![sample_entry("kick.wav", "Drums/kick.wav", false)],
    );
    assert_matches_fresh_build(&runner, rebuilds);
    assert!(runner.menu.focus_item_key("sample.pick.0.0.Drums/kick.wav"));

    assert!(runner.menu.focus_item_key("sample.up.0.0"));
    press(&mut runner);
    assert_matches_fresh_build(&runner, rebuilds);
    assert!(runner
        .menu
        .current_group_path()
        .ends_with("> Sampler > S1 Browse"));

    send_sample_list(
        &mut runner,
        "",
        vec![sample_entry("pad.wav", "pad.wav", false)],
    );
    assert!(runner.menu.focus_item_key("sample.pick.0.0.pad.wav"));
    press(&mut runner);
    assert_matches_fresh_build(&runner, rebuilds);
    assert_eq!(runner.menu.current_key(), Some("sample.choose:0:0"));
    assert!(runner.sample_browser.is_none());
}

#[test]
fn fx_slot_type_detents_update_their_picker_copies_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_detents_match_fresh_build(&mut runner, "mixer.buses.1.slot2.type", &[1, 2, -1]);
    assert_detents_match_fresh_build(&mut runner, "mixer.master.slots.0.type", &[1, 1, -1]);
    assert_ne!(runner.fx_buses[1].slot2_type, "none");
}
