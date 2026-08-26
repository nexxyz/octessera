use super::oled_output::{render_oled_if_changed, OledRenderDevice, OLED_RETRY_INTERVAL};
use super::*;
use crate::oled_frame_cache::OledFramePublication;
use platform_core::palette;
use playback_runtime::oled_frame::{runtime_error_rows, ERROR_ROW_COUNT, ERROR_ROW_WIDTH};
use playback_runtime::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(super) fn pixel(frame: &[u8], x: usize, y: usize) -> u16 {
    let idx = (y * 128 + x) * 2;
    u16::from_be_bytes([frame[idx], frame[idx + 1]])
}

pub(super) fn menu_snapshot() -> Value {
    json!({
        "display": {
            "off": false,
            "splash": "",
            "bodyLayout": "rows",
            "title": "Voice FX/Aux",
            "lines": ["  Volume +3", "@@ FX/Aux 1", "*Velocity", "  sample_1", "  (empty)", "  Q/V X", "  J+K"],
            "colors": [
                palette::WHITE_RGB565,
                palette::GREEN_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565,
                palette::WHITE_RGB565
            ],
            "barValues": [null, { "frac": 0.5 }, null, null, null, null, null],
            "scrollOffset": 2,
            "totalRows": 12,
            "visibleRows": 7,
            "toast": "",
            "editing": false
        },
        "settings": { "displayBrightness": 100, "autoSaveFlash": "none", "autoSaveFlashSerial": 0 },
        "selectedRow": 1,
        "transportIcon": "play",
        "transportFlash": "beat",
        "eventDotOn": true,
        "cpuLoadRatio": 0.0
    })
}

pub(super) fn producer_error(
    domain: RuntimeErrorDomain,
    code: RuntimeErrorCode,
    operation: RuntimeOperation,
    message: Option<&str>,
) -> Value {
    serde_json::to_value(RuntimeErrorMetadata::new(
        domain,
        code,
        operation,
        RuntimeRecovery::RetainLastGood,
        message.map(str::to_owned),
    ))
    .unwrap()
}

struct TestOled {
    failed_writes: usize,
    writes: usize,
    frames: Vec<Vec<u8>>,
    operations: Vec<&'static str>,
}

impl OledRenderDevice for TestOled {
    fn display_on(&mut self) -> Result<(), String> {
        self.operations.push("on");
        Ok(())
    }

    fn write_frame(&mut self, _frame: &[u8]) -> Result<(), String> {
        self.operations.push("write");
        self.writes += 1;
        self.frames.push(_frame.to_vec());
        if self.failed_writes == 0 {
            Ok(())
        } else {
            self.failed_writes -= 1;
            Err("test OLED write failed".into())
        }
    }

    fn display_off(&mut self) -> Result<(), String> {
        self.operations.push("off");
        Ok(())
    }
}

#[test]
fn runtime_error_frame_has_priority_over_splash_and_footer() {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["splash"] = json!("startup");
    snapshot["display"]["toast"] = json!("Saved");
    snapshot["runtimeError"] = json!({
        "domain": "sample",
        "code": "not_found",
        "operation": "audio_command",
        "recovery": "retain_last_good",
        "message": "sample not found"
    });

    let frame = oled_frame(&snapshot);

    assert_eq!(pixel(&frame, 0, 0), rgb565(dim(palette::RED, 6)));
    assert_eq!(pixel(&frame, 10, 10), palette::RED_RGB565);
    assert_eq!(pixel(&frame, 119, 119), rgb565(palette::BLACK));
}

#[test]
fn midi_input_runtime_error_uses_concise_native_presentation() {
    let mut snapshot = menu_snapshot();
    snapshot["display"]["title"] = json!("MIDI INPUTS");
    snapshot["display"]["lines"] = json!(["MIDI unavailable"]);
    snapshot["display"]["colors"] = json!([palette::WHITE_RGB565]);
    snapshot["display"]["barValues"] = json!([null]);
    snapshot["runtimeError"] = json!({
        "domain": "midi",
        "code": "operation_failed",
        "operation": "midi_list_inputs",
        "message": "ALSA details stay out of the OLED"
    });

    let mut frame = vec![0xa5_u8; OLED_FRAME_BYTES];
    oled_frame_into(&snapshot, &mut frame);

    assert_eq!(pixel(&frame, 0, 0), 0);
    assert_ne!(pixel(&frame, 5, 5), 0);
    assert_ne!(pixel(&frame, 6, 18), 0);
}

#[test]
fn runtime_error_layout_wraps_and_sanitizes_representative_producers() {
    for error in [
        producer_error(
            RuntimeErrorDomain::Audio,
            RuntimeErrorCode::Unavailable,
            RuntimeOperation::AudioCommand,
            Some("USB Audio Codec hw:2,0"),
        ),
        producer_error(
            RuntimeErrorDomain::Midi,
            RuntimeErrorCode::Unavailable,
            RuntimeOperation::MidiListInputs,
            Some("MIDI unavailable\nALSA details\tkept in logs"),
        ),
        producer_error(
            RuntimeErrorDomain::Sample,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::SampleList,
            Some("/home/pi/samples/very_long_name.wav"),
        ),
        producer_error(
            RuntimeErrorDomain::Sample,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::SamplePreview,
            Some("/mnt/🍄/音色/very_long_device_id.wav"),
        ),
        producer_error(
            RuntimeErrorDomain::Runtime,
            RuntimeErrorCode::Unavailable,
            RuntimeOperation::SetupPortal,
            Some("handoff timeout; try again"),
        ),
        producer_error(
            RuntimeErrorDomain::Runtime,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::DeviceUpdate,
            Some("health validation failed"),
        ),
        producer_error(
            RuntimeErrorDomain::Serialization,
            RuntimeErrorCode::InvalidPayload,
            RuntimeOperation::Snapshot,
            Some("bad\u{0000}metadata from runtime"),
        ),
        producer_error(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::StoreLoadDefault,
            Some("disk full while loading default preset"),
        ),
    ] {
        let rows = runtime_error_rows(&super::oled::runtime_error_metadata(&error));
        assert_eq!(rows.len(), ERROR_ROW_COUNT);
        assert!(rows
            .iter()
            .all(|row| row.chars().count() <= ERROR_ROW_WIDTH));
    }
}

#[test]
fn runtime_error_layout_ends_truncated_diagnostics_with_ellipsis() {
    let error = json!({
        "domain": "audio",
        "code": "device_missing",
        "operation": "open_default_device",
        "message": "this is a deliberately long diagnostic path that must not escape the OLED card"
    });
    let rows = runtime_error_rows(&super::oled::runtime_error_metadata(&error));
    assert!(rows
        .iter()
        .rev()
        .find(|row| !row.is_empty())
        .is_some_and(|row| row.ends_with("...")));
    assert!(rows
        .iter()
        .all(|row| row.chars().count() <= ERROR_ROW_WIDTH));
}

#[test]
fn runtime_error_frame_has_fixed_size_and_brightness_scaling() {
    let mut snapshot = menu_snapshot();
    snapshot["runtimeError"] = json!({
        "domain": "power",
        "code": "shutdown_failed",
        "operation": "request_shutdown",
        "message": "not accepted"
    });
    snapshot["settings"]["displayBrightness"] = json!(0);
    let dark = oled_frame(&snapshot);
    snapshot["settings"]["displayBrightness"] = json!(100);
    let bright = oled_frame(&snapshot);
    assert_eq!(dark.len(), OLED_FRAME_BYTES);
    assert_eq!(bright.len(), OLED_FRAME_BYTES);
    assert_ne!(dark, bright);
}

#[test]
fn failed_oled_write_does_not_advance_revision_before_retry_success() {
    let snapshot = serde_json::json!({"display": {"off": true}});
    let now = Instant::now();
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();

    let publication = OledFramePublication::ExplicitBlack;
    let retry_at =
        render_oled_if_changed(&mut oled, &snapshot, &publication, &mut cache, now).unwrap();

    assert_eq!(cache.oled_rendered_key, None);
    assert_eq!(oled.writes, 1);
    assert_eq!(retry_at, now + OLED_RETRY_INTERVAL);
    assert_eq!(
        render_oled_if_changed(
            &mut oled,
            &snapshot,
            &publication,
            &mut cache,
            now + Duration::from_millis(1),
        ),
        Some(now + OLED_RETRY_INTERVAL)
    );
    assert_eq!(oled.writes, 1);

    assert_eq!(
        render_oled_if_changed(
            &mut oled,
            &snapshot,
            &publication,
            &mut cache,
            now + OLED_RETRY_INTERVAL,
        ),
        None
    );
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(publication.key(), true))
    );
    assert_eq!(oled.writes, 2);
    assert!(oled
        .frames
        .iter()
        .all(|frame| frame.iter().all(|byte| *byte == 0)));
}

#[test]
fn accepted_native_frame_is_written_without_semantic_rendering() {
    let snapshot = serde_json::json!({
        "display": {"off": false},
        "settings": {"displayBrightness": 0}
    });
    let pixels = vec![0x5a; OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_native(7, pixels.clone());
    let mut oled = TestOled {
        failed_writes: 0,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();

    assert_eq!(
        render_oled_if_changed(
            &mut oled,
            &snapshot,
            &publication,
            &mut cache,
            Instant::now(),
        ),
        None
    );
    assert_eq!(oled.frames, vec![pixels]);
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(publication.key(), false))
    );
}

#[test]
fn newer_native_frame_supersedes_failed_retry_with_exact_bytes() {
    let snapshot = serde_json::json!({"display": {"off": false}});
    let old = OledFramePublication::test_native(1, vec![1; OLED_FRAME_BYTES]);
    let newer_pixels = vec![2; OLED_FRAME_BYTES];
    let newer = OledFramePublication::test_native(2, newer_pixels.clone());
    let now = Instant::now();
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    assert!(render_oled_if_changed(&mut oled, &snapshot, &old, &mut cache, now).is_some());
    assert_eq!(
        render_oled_if_changed(&mut oled, &snapshot, &newer, &mut cache, now),
        None
    );
    assert_eq!(oled.frames.last(), Some(&newer_pixels));
    assert_eq!(
        cache.oled_rendered_key,
        Some(OledOutputKey::new(newer.key(), false))
    );
}

#[test]
fn identical_black_frame_revision_reconciles_display_power_state() {
    let publication = OledFramePublication::test_native(7, vec![0; OLED_FRAME_BYTES]);
    let mut oled = TestOled {
        failed_writes: 0,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    let awake = serde_json::json!({"display": {"off": false}});
    let off = serde_json::json!({"display": {"off": true}});
    let now = Instant::now();

    assert_eq!(
        render_oled_if_changed(&mut oled, &awake, &publication, &mut cache, now),
        None
    );
    assert_eq!(
        render_oled_if_changed(&mut oled, &off, &publication, &mut cache, now),
        None
    );
    assert_eq!(
        render_oled_if_changed(&mut oled, &awake, &publication, &mut cache, now),
        None
    );

    assert_eq!(oled.operations, vec!["on", "write", "off", "on"]);
    assert_eq!(oled.writes, 1);
}

#[test]
fn retained_bytes_retry_across_display_transitions_stays_immutable() {
    let pixels = vec![0x5a; OLED_FRAME_BYTES];
    let publication = OledFramePublication::test_retained_last_good(7, pixels.clone());
    let mut oled = TestOled {
        failed_writes: 1,
        writes: 0,
        frames: Vec::new(),
        operations: Vec::new(),
    };
    let mut cache = HardwareRenderCache::default();
    let awake = serde_json::json!({"display": {"off": false}});
    let off = serde_json::json!({"display": {"off": true}});
    let now = Instant::now();

    assert!(render_oled_if_changed(&mut oled, &awake, &publication, &mut cache, now).is_some());
    assert_eq!(
        render_oled_if_changed(
            &mut oled,
            &off,
            &publication,
            &mut cache,
            now + OLED_RETRY_INTERVAL,
        ),
        None
    );
    assert_eq!(
        render_oled_if_changed(&mut oled, &awake, &publication, &mut cache, now),
        None
    );

    assert_eq!(oled.operations, vec!["on", "write", "write", "off", "on"]);
    assert_eq!(oled.frames, vec![pixels.clone(), pixels]);
}
