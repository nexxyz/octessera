use super::*;
use realtime_engine::synth::{
    normalize_instrument_slot_config, InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig,
    SampleBuffer, SampleSlotConfig, SynthEngine, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
    SAMPLE_SLOTS_PER_INSTRUMENT,
};

const SAMPLER: [&str; 4] = ["Shape", "Instruments", "I1: Sampler direct", "Sampler"];

fn sampler_runner(note_behavior: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].name = "sampler".into();
    runner.instruments[0].note_behavior = note_behavior.into();
    runner.instruments[0].sample_paths[0] = Some("samples/loop.wav".into());
    runner.menu.rebuild(runner.menu_config());
    runner.menu.open_labels(&SAMPLER);
    runner
}

fn input(runner: &mut NativeRunner, input: Value) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap()
}

/// Presses into the row, turns it with Fn held (one step per detent) and presses out.
fn edit_row(runner: &mut NativeRunner, label: &str, delta: i32) {
    runner.menu.select_label(label);
    for event in [
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "button_fn", "pressed": true }),
        json!({ "type": "encoder_turn", "id": "main", "delta": delta }),
        json!({ "type": "button_fn", "pressed": false }),
        json!({ "type": "encoder_press", "id": "main" }),
    ] {
        let _ = input(runner, event);
    }
}

fn saved_slot(runner: &NativeRunner, slot: usize) -> Value {
    runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["slots"][slot].clone()
}

fn looping_runner() -> NativeRunner {
    let mut runner = sampler_runner("hold");
    edit_row(&mut runner, "Loop", 1);
    edit_row(&mut runner, "Start", 20);
    edit_row(&mut runner, "Loop Start", 30);
    edit_row(&mut runner, "Loop End", -30);
    runner
}

#[test]
fn device_input_sets_start_and_loop_rows_for_the_selected_slot() {
    let mut runner = sampler_runner("hold");
    assert!(!runner
        .test_menu_sibling_labels()
        .contains(&"Loop Start".to_string()));
    edit_row(&mut runner, "Loop", 1);
    assert!(runner
        .test_menu_sibling_labels()
        .contains(&"Loop End".to_string()));
    edit_row(&mut runner, "Start", 20);
    edit_row(&mut runner, "Loop Start", 30);
    edit_row(&mut runner, "Loop End", -30);
    assert_eq!(
        saved_slot(&runner, 0),
        json!({
            "path": "samples/loop.wav",
            "loop": "forward",
            "startPct": 20,
            "loopStartPct": 30,
            "loopEndPct": 70
        })
    );

    edit_row(&mut runner, "Sample Slot", 1);
    assert!(!runner
        .test_menu_sibling_labels()
        .contains(&"Loop Start".to_string()));
    edit_row(&mut runner, "Loop", 2);
    assert_eq!(saved_slot(&runner, 1)["loop"], "pingpong");
    assert_eq!(saved_slot(&runner, 1)["loopEndPct"], 100);
    assert_eq!(saved_slot(&runner, 0)["loop"], "forward");
    assert_eq!(
        runner.instrument_audio_config(0).unwrap()["sample"]["slots"][1]["loop"],
        "pingpong"
    );
}

#[test]
fn loop_start_and_end_stop_one_percent_apart() {
    let mut runner = looping_runner();
    edit_row(&mut runner, "Loop Start", 90);
    assert_eq!(saved_slot(&runner, 0)["loopStartPct"], 69);
    assert_eq!(
        runner
            .menu
            .number_for_key("instruments.0.sample.slots.0.loopStartPct"),
        Some(69)
    );
    edit_row(&mut runner, "Loop End", -90);
    assert_eq!(saved_slot(&runner, 0)["loopEndPct"], 70);
    edit_row(&mut runner, "Loop Start", -90);
    edit_row(&mut runner, "Loop End", -90);
    assert_eq!(saved_slot(&runner, 0)["loopEndPct"], 1);
}

#[test]
fn slot_loops_survive_a_local_save_and_reload() {
    let runner = looping_runner();
    let documents =
        crate::native_runner::split_local_system_patch_documents(&runner.config_payload()).unwrap();
    let full = crate::native_runner::compose_local_system_patch_documents(
        &documents.system,
        &documents.patch,
    )
    .unwrap();
    let mut reloaded = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    reloaded.apply_config_payload(full).unwrap();

    assert_eq!(saved_slot(&reloaded, 0), saved_slot(&runner, 0));
    assert_eq!(
        reloaded.instruments[0].sample_playback[0].loop_mode,
        "forward"
    );

    let mut missing = documents.patch.clone();
    missing["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]
        .as_object_mut()
        .unwrap()
        .remove("loopEndPct");
    let error =
        crate::native_runner::compose_local_system_patch_documents(&documents.system, &missing)
            .unwrap_err();
    assert!(error.contains("loopEndPct is required"), "{error}");
}

#[test]
fn saved_slot_loops_are_validated() {
    let runner = looping_runner();
    for (key, value) in [
        ("loop", json!("backwards")),
        ("startPct", json!(100)),
        ("loopEndPct", json!(30)),
    ] {
        let mut payload = runner.config_payload();
        payload["runtimeConfig"]["instruments"][0]["sample"]["slots"][0][key] = value;
        let mut fresh = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        assert!(fresh.apply_config_payload(payload).is_err(), "{key}");
    }
}

fn engine_for(audio_config: &Value, frames: usize) -> SynthEngine {
    let instrument = normalize_instrument_slot_config(audio_config).unwrap();
    let mut slots = vec![SampleSlotConfig::default(); SAMPLE_SLOTS_PER_INSTRUMENT];
    slots[0].buffer = Some(SampleBuffer {
        samples: (0..frames)
            .map(|index| (index as f32 * 0.05).sin() * 0.5)
            .collect::<Vec<_>>()
            .into(),
        channels: 1,
        sample_rate: 48_000,
    });
    let mut banks = vec![instrument.active_sample().unwrap().bank(slots)];
    banks.resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![
            InstrumentSlotConfig {
                kind: "sampler".into(),
                ..instrument.slot.clone()
            };
            INSTRUMENT_SLOT_COUNT
        ],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let _ = engine.set_sample_banks(banks);
    engine
}

fn sounding_after(engine: &mut SynthEngine, frames: usize) -> bool {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut last = Vec::new();
    for _ in 0..frames.div_ceil(128) {
        engine.render_interleaved_block(128, &mut left, &mut right, &mut out);
        last.clone_from(&out);
    }
    last.iter().any(|value| *value != 0.0)
}

#[test]
fn a_held_looping_note_plays_past_the_sample_until_released() {
    let runner = looping_runner();
    let mut engine = engine_for(&runner.instrument_audio_config(0).unwrap(), 1_000);
    engine.tile_hit(0, 0, 0, 0, 100);
    assert!(sounding_after(&mut engine, 4_000));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 1);
    engine.tile_hit(0, 0, 0, 0, 0);
    assert!(!sounding_after(&mut engine, 1_024));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}

#[test]
fn oneshot_ignores_a_saved_loop_and_switching_note_mode_resends_the_slot() {
    let mut runner = looping_runner();
    runner
        .menu
        .open_labels(&["Shape", "Instruments", "I1: Sampler direct"]);
    runner.menu.select_label("Note Mode");
    let messages = [
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "encoder_turn", "id": "main", "delta": -1 }),
        json!({ "type": "encoder_press", "id": "main" }),
    ]
    .into_iter()
    .flat_map(|event| input(&mut runner, event))
    .collect::<Vec<_>>();
    assert_eq!(runner.instruments[0].note_behavior, "oneshot");
    assert!(messages.iter().any(|message| matches!(message,
        RunnerMessage::AudioCommands { commands } if commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetInstrumentSlot { instrument_slot: 0, config, .. }
                if config["noteBehavior"] == "oneshot"
        ))
    )));
    let mut engine = engine_for(&runner.instrument_audio_config(0).unwrap(), 1_000);
    engine.tile_hit(0, 0, 0, 0, 100);
    assert!(!sounding_after(&mut engine, 4_000));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}
