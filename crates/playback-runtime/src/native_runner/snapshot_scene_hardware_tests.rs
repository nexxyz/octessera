use super::super::{NativeConfirmDialog, NativeHelpPopup, NativeRuntimeErrorPresentation};
use super::*;
use crate::native_menu::NativeMenuAction;
use crate::oled_frame::{presentation_input_from_snapshot, render_oled_frame};
use serde_json::json;

fn shipped_runner() -> NativeRunner {
    let payload: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(super::super::NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    runner
}

fn bytes(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u8)
        .collect()
}

fn active(value: &Value) -> Vec<bool> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_bool().unwrap())
        .collect()
}

fn verify_hardware(
    runner: &NativeRunner,
    metrics: OledPresentationMetrics,
    error: Option<OledRuntimeErrorMetadata>,
) -> NativeHardwarePresentation {
    let scene = runner.capture_presentation_scene(false).unwrap();
    let generation = scene.generation();
    fn assert_send<T: Send>() {}
    assert_send::<PresentationScene>();
    assert_send::<NativeHardwarePresentation>();
    let hardware =
        std::thread::spawn(move || scene.into_hardware_presentation(metrics.clone(), error))
            .join()
            .unwrap();
    let mut legacy = runner.snapshot().unwrap();
    if let Some(error) = &hardware.oled.runtime_error {
        legacy["runtimeError"] = json!({"domain": error.domain, "code": error.code,
            "operation": error.operation, "message": error.message});
    }
    let expected = presentation_input_from_snapshot(&legacy, hardware.oled.metrics.clone())
        .unwrap()
        .unwrap();
    assert_eq!(hardware.generation, generation);
    assert_eq!(hardware.oled, expected);
    assert_eq!(
        render_oled_frame(&hardware.oled),
        render_oled_frame(&expected)
    );
    assert_eq!(hardware.leds.grid.rgb, bytes(&legacy["leds"]["rgb"]));
    assert_eq!(hardware.leds.grid.active, active(&legacy["leds"]["active"]));
    assert_eq!(
        hardware.leds.grid.width,
        legacy["leds"]["width"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hardware.leds.grid.height,
        legacy["leds"]["height"].as_u64().unwrap() as usize
    );
    assert_eq!(
        hardware.leds.brightness,
        legacy["settings"]["gridBrightness"]
    );
    assert_eq!(hardware.leds.dimmed, legacy["settings"]["ledsDimmed"]);
    assert_eq!(hardware.leds.off, legacy["display"]["off"]);
    assert_eq!(
        hardware.neo_key.brightness,
        legacy["settings"]["buttonBrightness"]
    );
    for (key, color) in [
        ("back", hardware.neo_key.colors[0]),
        ("space", hardware.neo_key.colors[1]),
        ("shift", hardware.neo_key.colors[2]),
        ("fn", hardware.neo_key.colors[3]),
    ] {
        assert_eq!(color.to_vec(), bytes(&legacy["neoKeyLeds"][key]));
    }
    assert_eq!(
        hardware.hdmi.mode.as_str(),
        legacy["hdmi"]["mode"].as_str().unwrap()
    );
    assert_eq!(
        hardware.hdmi.show_gridlines,
        legacy["hdmi"]["showGridlines"]
    );
    assert_eq!(
        hardware.hdmi.cycle_measures,
        legacy["hdmi"]["cycleMeasures"]
    );
    assert_eq!(
        hardware.hdmi.source_layer_index,
        legacy["hdmi"]["sourceLayerIndex"]
    );
    assert_eq!(
        hardware.hdmi.source_behavior_id,
        legacy["hdmi"]["sourceBehaviorId"]
    );
    assert_eq!(
        hardware.hdmi.grid.rgb,
        bytes(&legacy["hdmi"]["grid"]["rgb"])
    );
    assert_eq!(
        hardware.hdmi.grid.active,
        active(&legacy["hdmi"]["grid"]["active"])
    );
    hardware
}

#[test]
fn shipped_default_modes_preserve_native_pixels_and_source() {
    let mut runner = shipped_runner();
    for (name, mode) in [
        ("none", NativeHdmiMode::None),
        ("live-grid", NativeHdmiMode::LiveGrid),
        ("plain-grid", NativeHdmiMode::PlainGrid),
        ("active-behavior", NativeHdmiMode::ActiveBehavior),
        ("cycle-behaviors", NativeHdmiMode::CycleBehaviors),
    ] {
        runner.display.hdmi.mode = name.into();
        let hardware = verify_hardware(&runner, OledPresentationMetrics::default(), None);
        assert_eq!(hardware.hdmi.mode, mode);
        assert_eq!(hardware.leds.brightness, 25);
        assert_eq!(hardware.neo_key.brightness, 35);
        assert!(!hardware.leds.off);
        let alive = (super::super::GRID_HEIGHT - 1 - 1) * super::super::GRID_WIDTH + 2;
        assert_eq!(
            &hardware.leds.grid.rgb[alive * 3..alive * 3 + 3],
            &[255, 255, 0]
        );
        assert!(hardware.leds.grid.active[alive]);
        if matches!(
            mode,
            NativeHdmiMode::PlainGrid
                | NativeHdmiMode::ActiveBehavior
                | NativeHdmiMode::CycleBehaviors
        ) {
            assert_eq!(
                &hardware.hdmi.grid.rgb[alive * 3..alive * 3 + 3],
                &[255, 255, 0]
            );
            assert!(hardware.hdmi.grid.active[alive]);
            assert_eq!(hardware.hdmi.source_layer_index, 0);
            assert_eq!(hardware.hdmi.source_behavior_id, "life");
        }
        if mode == NativeHdmiMode::None {
            assert!(hardware.hdmi.grid.rgb.iter().all(|byte| *byte == 0));
        }
    }
    runner.display.hdmi.mode = "cycle-behaviors".into();
    runner.transport.current_ppqn_pulse = 4 * 96;
    let cycled = verify_hardware(&runner, OledPresentationMetrics::default(), None);
    assert_eq!(cycled.hdmi.source_layer_index, 1);
    assert_eq!(cycled.hdmi.source_behavior_id, "sequencer");
    let sequencer_cell = (super::super::GRID_HEIGHT - 1 - 1) * super::super::GRID_WIDTH + 2;
    assert!(cycled.hdmi.grid.active[sequencer_cell]);
    assert_eq!(
        &cycled.hdmi.grid.rgb[sequencer_cell * 3..sequencer_cell * 3 + 3],
        &[255, 212, 71]
    );
    assert_ne!(
        &cycled.hdmi.grid.rgb[sequencer_cell * 3..sequencer_cell * 3 + 3],
        &[255, 255, 0]
    );
    runner.display.hdmi.mode = "active-behavior".into();
    runner.display.hdmi.source_layer_index = 4;
    let empty_source = verify_hardware(&runner, OledPresentationMetrics::default(), None);
    assert_eq!(empty_source.hdmi.mode, NativeHdmiMode::ActiveBehavior);
    assert_eq!(empty_source.hdmi.source_behavior_id, "none");
    assert!(empty_source.hdmi.grid.rgb.iter().all(|value| *value == 0));
}

#[test]
fn hardware_oled_off_and_error_leave_grid_and_hdmi_live() {
    let mut runner = shipped_runner();
    runner.display.hdmi.mode = "plain-grid".into();
    let awake = verify_hardware(&runner, OledPresentationMetrics::default(), None);
    let metadata = OledRuntimeErrorMetadata {
        domain: Some("runtime".into()),
        code: Some("operation_failed".into()),
        operation: Some("runtime_dispatch".into()),
        message: Some("Keep playing".into()),
    };
    runner.display.help_popup = Some(NativeHelpPopup {
        title: "Help".into(),
        lines: vec!["Open menu".into()],
        scroll: 0,
    });
    runner.display.confirm_dialog = Some(NativeConfirmDialog {
        title: "Confirm".into(),
        lines: vec!["Proceed?".into()],
        options: vec!["Cancel".into(), "Confirm".into()],
        cursor: 1,
        action: NativeMenuAction::NavigateBack,
        cancel_toast: None,
        confirm_before_execute: false,
    });
    runner.display.runtime_error_presentation = Some(NativeRuntimeErrorPresentation {
        title: "Fault".into(),
        lines: vec!["Keep playing".into()],
    });
    let priority = verify_hardware(
        &runner,
        OledPresentationMetrics::default(),
        Some(metadata.clone()),
    );
    assert_eq!(priority.oled.display.title, "Fault");
    assert_eq!(priority.oled.runtime_error, Some(metadata.clone()));
    runner.display.runtime_error_presentation = None;
    runner.display.confirm_dialog = None;
    runner.display.help_popup = None;
    runner.display.oled_mode = NativeOledMode::Off;
    let off = verify_hardware(
        &runner,
        OledPresentationMetrics::from_status(Some(0.9), true, true, true),
        Some(metadata.clone()),
    );
    assert!(off.oled.display.off);
    assert!(off.leds.off);
    assert_eq!(off.oled.runtime_error, Some(metadata));
    assert_eq!(off.leds.grid.rgb, awake.leds.grid.rgb);
    assert_eq!(off.hdmi.grid.rgb, awake.hdmi.grid.rgb);
}
