use super::audio_restart_persistence::{enter_edit, press, turn};
use super::in_place_menu_rows::{assert_matches_fresh_build, runner_with_instrument};
use super::*;

fn send_store_result(runner: &mut NativeRunner, result: RuntimeStoreResult) {
    runner.send(HostMessage::RuntimeResult { result }).unwrap();
}

fn input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

pub(super) fn press_key(runner: &mut NativeRunner, key: &str) {
    assert!(runner.menu.focus_item_key(key), "{key}");
    press(runner);
}

pub(super) fn confirm_key(runner: &mut NativeRunner, key: &str) -> Vec<RunnerMessage> {
    press_key(runner, key);
    assert!(runner.display.confirm_dialog.is_some(), "{key}");
    turn(runner, 1);
    press(runner)
}

#[test]
fn startup_builds_the_menu_once() {
    let runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();

    assert_matches_fresh_build(&runner, 1);
}

#[test]
fn preset_and_default_loads_build_the_menu_once() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    send_store_result(
        &mut runner,
        RuntimeStoreResult::LoadPresetResult {
            name: "Seq".into(),
            payload: Some(json!({
                "runtimeConfig": {
                    "activeBehavior": "sequencer",
                    "layers": [{ "build": { "behaviorId": "sequencer" } }]
                }
            })),
        },
    );
    assert_eq!(runner.behavior.id(), "sequencer");
    assert_matches_fresh_build(&runner, 1);

    let payload = portable_patch_payload_for_save(
        &NativeRunner::new(NativeRunnerConfig::default())
            .unwrap()
            .config_payload(),
    )
    .unwrap();
    send_store_result(
        &mut runner,
        RuntimeStoreResult::LoadDefaultResult {
            payload: Some(payload),
        },
    );
    assert_eq!(runner.behavior.id(), "life");
    assert_matches_fresh_build(&runner, 1);
}

#[test]
fn preset_catalog_results_update_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;

    send_store_result(
        &mut runner,
        RuntimeStoreResult::ListPresetsResult {
            names: vec!["Alpha".into(), "Beta".into()],
        },
    );
    assert_matches_fresh_build(&runner, rebuilds);
    runner.preset_names.push("Gamma".into());
    send_store_result(
        &mut runner,
        RuntimeStoreResult::SavePresetResult {
            name: "Gamma".into(),
            outcome: "saved".into(),
        },
    );
    assert_eq!(runner.current_preset_name.as_deref(), Some("Gamma"));
    assert_matches_fresh_build(&runner, rebuilds);
}

#[test]
fn mapping_target_pick_and_clear_update_the_picker_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;

    for target in [
        "xy:x",
        "param:0:x:1",
        "linkLfos.0.target",
        "aux:0:turn",
        "shiftAux:1:turn",
    ] {
        press_key(&mut runner, &format!("{target}.instruments.0.mixer.volume"));
        assert_matches_fresh_build(&runner, rebuilds);
        assert_eq!(runner.menu.current_key(), Some(target));
        press_key(&mut runner, &format!("{target}.none"));
        assert_matches_fresh_build(&runner, rebuilds);
    }
}

#[test]
fn mapping_target_changes_update_their_dependent_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;

    press_key(&mut runner, "linkLfos.0.target.instruments.0.mixer.volume");
    enter_edit(&mut runner, "linkLfos.0.enabled");
    turn(&mut runner, 1);
    press(&mut runner);
    assert!(runner.link_lfos[0].enabled);
    press_key(&mut runner, "linkLfos.0.target.none");
    assert!(!runner.link_lfos[0].enabled);
    assert_matches_fresh_build(&runner, rebuilds);

    press_key(&mut runner, "param:0:x:0.instruments.0.mixer.volume");
    enter_edit(&mut runner, "layers.0.paramMods.x.0.invert");
    turn(&mut runner, 1);
    press(&mut runner);
    assert!(runner.param_mods[0].x[0].as_ref().unwrap().invert);
    press_key(&mut runner, "param:0:x:0.instruments.0.mixer.panPos");
    assert_matches_fresh_build(&runner, rebuilds);
}

#[test]
fn grid_param_mod_mapping_updates_the_slot_pickers_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;
    assert!(runner.menu.focus_item_key("instruments.0.mixer.volume"));

    input(
        &mut runner,
        json!({ "type": "button_shift", "pressed": true }),
    );
    for invert in [false, true] {
        input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 0 }));
        input(
            &mut runner,
            json!({ "type": "grid_release", "x": 0, "y": 0 }),
        );
        assert_eq!(runner.param_mods[0].x[0].as_ref().unwrap().invert, invert);
        assert_matches_fresh_build(&runner, rebuilds);
    }
    input(&mut runner, json!({ "type": "grid_press", "x": 0, "y": 0 }));
    assert!(runner.param_mods[0].x[0].is_none());
    assert_matches_fresh_build(&runner, rebuilds);
    assert_eq!(
        runner.menu.current_key(),
        Some("instruments.0.mixer.volume")
    );
}

fn instrument_slot_commands(messages: &[RunnerMessage]) -> Vec<usize> {
    messages
        .iter()
        .flat_map(|message| match message {
            RunnerMessage::AudioCommands { commands } => commands.iter().collect(),
            _ => Vec::new(),
        })
        .filter_map(|command| match command {
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot, ..
            } => Some(*instrument_slot),
            _ => None,
        })
        .collect()
}

#[test]
fn clone_and_reset_instrument_update_menu_and_audio() {
    let mut runner = runner_with_instrument("fm");
    let target = 5;
    runner.instruments[target].kind = "none".into();
    runner.instruments[target].name = derive_instrument_name(target, "none");
    runner.menu.rebuild(runner.menu_config());
    let rebuilds = runner.menu.rebuild_count;
    let cloned = confirm_key(&mut runner, "instruments.0.clone");
    assert_eq!(runner.instruments[target].kind, "fm");
    assert_eq!(instrument_slot_commands(&cloned), vec![target]);
    assert_matches_fresh_build(&runner, rebuilds);

    let reset = confirm_key(&mut runner, "instruments.0.reset");
    assert_eq!(runner.instruments[0], NativeInstrumentSlot::reset(0));
    assert_eq!(instrument_slot_commands(&reset), vec![0]);
    assert_matches_fresh_build(&runner, rebuilds);
}

#[test]
fn factory_sound_and_synth_preset_loads_update_their_instrument_in_place() {
    for (kind, key) in [
        ("fm", "fm.preset.2.bell"),
        ("drum", "drum.kit.2.tight"),
        ("sampler", "sample.kit.2.synthkit"),
        ("synth", "synth.preset.2.bell"),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.instruments[2].kind = kind.into();
        runner.instruments[2].name = derive_instrument_name(2, kind);
        runner.menu.rebuild(runner.menu_config());
        let rebuilds = runner.menu.rebuild_count;

        let loaded = confirm_key(&mut runner, key);
        assert_eq!(instrument_slot_commands(&loaded), vec![2], "{key}");
        assert_matches_fresh_build(&runner, rebuilds);
    }
}

#[test]
fn instrument_type_route_and_auto_name_edits_update_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;

    enter_edit(&mut runner, "instruments.1.mixer.route");
    turn(&mut runner, 2);
    press(&mut runner);
    assert_ne!(runner.instruments[1].route, "direct");
    assert_matches_fresh_build(&runner, rebuilds);

    enter_edit(&mut runner, "instruments.1.autoName");
    turn(&mut runner, -1);
    press(&mut runner);
    assert!(!runner.instruments[1].auto_name);
    assert_matches_fresh_build(&runner, rebuilds);

    for (auto_name, delta) in [(false, 1), (true, 4)] {
        if auto_name {
            enter_edit(&mut runner, "instruments.1.autoName");
            turn(&mut runner, 1);
            press(&mut runner);
        }
        let name = runner.instruments[1].name.clone();
        enter_edit(&mut runner, "instruments.1.type");
        turn(&mut runner, delta);
        press(&mut runner);
        assert_eq!(runner.menu.current_key(), Some("instruments.1.type"));
        assert_eq!(runner.instruments[1].name == name, !auto_name);
        assert_matches_fresh_build(&runner, rebuilds);
    }
    assert_eq!(runner.instruments[1].kind, "drum");
}

#[test]
fn fx_bus_auto_name_updates_the_bus_rows_in_place() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let rebuilds = runner.menu.rebuild_count;

    enter_edit(&mut runner, "mixer.buses.1.autoName");
    turn(&mut runner, -1);
    press(&mut runner);
    enter_edit(&mut runner, "mixer.buses.1.slot1.type");
    turn(&mut runner, 2);
    press(&mut runner);
    let name = runner.fx_buses[1].name.clone();
    enter_edit(&mut runner, "mixer.buses.1.autoName");
    turn(&mut runner, 1);
    press(&mut runner);
    assert_ne!(runner.fx_buses[1].name, name);
    assert_eq!(runner.menu.current_key(), Some("mixer.buses.1.autoName"));
    assert_matches_fresh_build(&runner, rebuilds);
}

#[test]
fn sample_favourites_and_missing_samples_update_their_instrument_in_place() {
    let mut runner = runner_with_instrument("sampler");
    runner.instruments[0].sample_paths[1] = Some("Drums/snare.wav".into());
    runner.menu.rebuild(runner.menu_config());
    let rebuilds = runner.menu.rebuild_count;

    press_key(&mut runner, "sample.choose:0:0");
    send_store_result(
        &mut runner,
        RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "".into(),
            entries: vec![SampleEntry {
                name: "Pads".into(),
                path: "Pads".into(),
                is_dir: true,
            }],
        },
    );
    press_key(&mut runner, "sample.enter.0.0.Pads");
    for key in ["sample.favorite.set.0.0", "sample.favorite.remove.0.0"] {
        press_key(&mut runner, key);
        assert_matches_fresh_build(&runner, rebuilds);
    }

    send_store_result(
        &mut runner,
        RuntimeStoreResult::SamplesMissing {
            paths: vec!["Drums/snare.wav".into()],
        },
    );
    assert_eq!(
        runner.sample_availability[0][1],
        NativeSampleAvailability::Unavailable
    );
    assert_matches_fresh_build(&runner, rebuilds);

    runner.sample_availability[0][1] = NativeSampleAvailability::Available;
    runner.menu.rebuild(runner.menu_config());
    send_store_result(
        &mut runner,
        RuntimeStoreResult::RuntimeFailure {
            error: crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Sample,
                crate::RuntimeErrorCode::NotFound,
                crate::RuntimeOperation::AudioCommand,
                Some("sample not found: Drums/snare.wav".into()),
            ),
        },
    );
    assert_eq!(
        runner.sample_availability[0][1],
        NativeSampleAvailability::Unavailable
    );
    assert_matches_fresh_build(&runner, rebuilds + 1);
}
