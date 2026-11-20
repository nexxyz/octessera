use super::error_layout::{
    runtime_error_rows, runtime_error_screen, ERROR_OPERATION_ROW, ERROR_ROW_COUNT, ERROR_ROW_WIDTH,
};
use super::font_5x7::glyph_rows;
use super::model::OledRuntimeErrorMetadata;
use crate::{
    RuntimeErrorCode, RuntimeErrorDomain, RuntimeErrorMetadata, RuntimeOperation, RuntimeRecovery,
};
use serde::Serialize;

fn metadata(
    domain: Option<&str>,
    code: Option<&str>,
    operation: Option<&str>,
    message: Option<&str>,
) -> OledRuntimeErrorMetadata {
    OledRuntimeErrorMetadata {
        domain: domain.map(str::to_owned),
        code: code.map(str::to_owned),
        operation: operation.map(str::to_owned),
        message: message.map(str::to_owned),
    }
}

fn runtime_metadata(
    domain: RuntimeErrorDomain,
    code: RuntimeErrorCode,
    operation: RuntimeOperation,
    message: Option<&str>,
) -> OledRuntimeErrorMetadata {
    let error = RuntimeErrorMetadata::new(
        domain,
        code,
        operation,
        RuntimeRecovery::RetainLastGood,
        message.map(str::to_owned),
    );
    metadata(
        Some(&enum_text(&error.domain)),
        Some(&enum_text(&error.code)),
        Some(&enum_text(&error.operation)),
        error.message.as_deref(),
    )
}

fn enum_text<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn producer_corpus() -> [(&'static str, OledRuntimeErrorMetadata); 11] {
    [
        (
            "audio-command",
            runtime_metadata(
                RuntimeErrorDomain::Audio,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::AudioCommand,
                Some("USB Audio Codec hw:2,0"),
            ),
        ),
        (
            "midi-inputs",
            runtime_metadata(
                RuntimeErrorDomain::Midi,
                RuntimeErrorCode::Unavailable,
                RuntimeOperation::MidiListInputs,
                Some("MIDI unavailable\nALSA details\tkept in logs"),
            ),
        ),
        (
            "midi-message",
            runtime_metadata(
                RuntimeErrorDomain::Midi,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::MidiMessage,
                Some("external MIDI write failed"),
            ),
        ),
        (
            "sample-list",
            runtime_metadata(
                RuntimeErrorDomain::Sample,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::SampleList,
                Some("/home/pi/samples/very_long_name.wav"),
            ),
        ),
        (
            "sample-preview",
            runtime_metadata(
                RuntimeErrorDomain::Sample,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::SamplePreview,
                Some("/mnt/🍄/音色/very_long_device_id.wav"),
            ),
        ),
        (
            "storage-default",
            runtime_metadata(
                RuntimeErrorDomain::Storage,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::StoreLoadDefault,
                Some("disk full while loading default preset"),
            ),
        ),
        (
            "snapshot",
            runtime_metadata(
                RuntimeErrorDomain::Serialization,
                RuntimeErrorCode::InvalidPayload,
                RuntimeOperation::Snapshot,
                Some("bad\u{0000}metadata from runtime"),
            ),
        ),
        (
            "setup-portal",
            runtime_metadata(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::Unavailable,
                RuntimeOperation::SetupPortal,
                Some("handoff timeout; try again"),
            ),
        ),
        (
            "device-update",
            runtime_metadata(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::DeviceUpdate,
                Some("health validation failed"),
            ),
        ),
        (
            "system-info",
            runtime_metadata(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::Unavailable,
                RuntimeOperation::SystemInfo,
                Some("system information unavailable"),
            ),
        ),
        (
            "runtime-dispatch",
            runtime_metadata(
                RuntimeErrorDomain::Runtime,
                RuntimeErrorCode::OperationFailed,
                RuntimeOperation::RuntimeDispatch,
                Some("runner failed"),
            ),
        ),
    ]
}

fn screen_of(error: &OledRuntimeErrorMetadata, preset_name: Option<&str>) -> (String, String) {
    let screen = runtime_error_screen(error, preset_name);
    (screen.title.to_owned(), screen.text)
}

fn rows_of(error: &OledRuntimeErrorMetadata) -> [String; ERROR_ROW_COUNT] {
    runtime_error_rows(
        &runtime_error_screen(error, None).text,
        error.operation.as_deref(),
    )
}

fn failure(
    domain: RuntimeErrorDomain,
    code: RuntimeErrorCode,
    operation: RuntimeOperation,
) -> OledRuntimeErrorMetadata {
    runtime_metadata(domain, code, operation, Some("x"))
}

fn store_failure(operation: RuntimeOperation, code: RuntimeErrorCode) -> OledRuntimeErrorMetadata {
    failure(RuntimeErrorDomain::Storage, code, operation)
}

#[test]
fn typed_producer_corpus_has_exact_bounded_supported_rows() {
    for (name, error) in producer_corpus() {
        let rows = rows_of(&error);
        assert_eq!(rows.len(), ERROR_ROW_COUNT, "row count: {name}");
        assert!(rows.iter().all(|row| {
            row.chars().count() <= ERROR_ROW_WIDTH
                && !row.chars().any(char::is_control)
                && row
                    .chars()
                    .all(|character| character == ' ' || glyph_rows(character) != [0; 7])
        }));
    }
}

#[test]
fn rows_hold_wrapped_text_a_blank_row_and_the_operation_id() {
    let rows = rows_of(&store_failure(
        RuntimeOperation::StoreSaveDefault,
        RuntimeErrorCode::OperationFailed,
    ));
    assert_eq!(
        rows,
        [
            "Your patch was not".to_owned(),
            "saved. Check the".to_owned(),
            "SD card, then try".to_owned(),
            "again.".to_owned(),
            String::new(),
            "store_save_default".to_owned(),
            String::new(),
        ]
    );
    assert_eq!(ERROR_OPERATION_ROW, 5);
}

#[test]
fn the_raw_message_never_reaches_the_screen() {
    for (name, error) in producer_corpus() {
        let message = error.message.clone().unwrap();
        let shown = rows_of(&error).join(" ");
        assert!(!shown.contains(message.lines().next().unwrap()), "{name}");
    }
}

#[test]
fn each_family_has_its_title_and_an_accurate_line() {
    use RuntimeErrorCode::{
        AudioThreadFailed, InvalidPayload, NotFound, OperationFailed, Unavailable, Unsupported,
    };
    use RuntimeErrorDomain::{Audio, Midi, Recording, Runtime, Sample};
    use RuntimeOperation::*;
    let cases = [
        (
            store_failure(StoreSaveSystem, OperationFailed),
            "CAN'T SAVE",
            "Settings were not saved. Check the SD card, then try again.",
        ),
        (
            store_failure(StoreSaveBackup, OperationFailed),
            "CAN'T SAVE",
            "A backup copy was not saved. Check the SD card, then try again.",
        ),
        (
            store_failure(StoreSaveRecovery, OperationFailed),
            "CAN'T SAVE",
            "Your patch was not saved. Check the SD card, then try again.",
        ),
        (
            store_failure(StoreSaveDefault, InvalidPayload),
            "CAN'T SAVE",
            "Your patch was not saved. Its data wasn't valid.",
        ),
        (
            store_failure(StoreLoadDefault, OperationFailed),
            "CAN'T LOAD",
            "Your saved patch didn't load. The patch in use hasn't changed.",
        ),
        (
            store_failure(StoreLoadSystem, OperationFailed),
            "CAN'T LOAD",
            "Settings didn't load. The settings in use haven't changed.",
        ),
        (
            failure(Midi, OperationFailed, MidiMessage),
            "MIDI TROUBLE",
            "Couldn't send to the MIDI device. Check the cable.",
        ),
        (
            failure(Midi, OperationFailed, MidiListOutputs),
            "MIDI TROUBLE",
            "Couldn't list the MIDI outputs. Try again.",
        ),
        (
            failure(Audio, AudioThreadFailed, AudioThread),
            "AUDIO TROUBLE",
            "Sound stopped. Check the audio output, then press Play.",
        ),
        (
            failure(Audio, OperationFailed, AudioCommand),
            "AUDIO TROUBLE",
            "An audio command failed. Check the audio output.",
        ),
        (
            failure(Sample, OperationFailed, SampleList),
            "SAMPLE TROUBLE",
            "Couldn't list the samples. Check the SD card.",
        ),
        (
            failure(Sample, NotFound, SamplePreview),
            "SAMPLE TROUBLE",
            "That sample wasn't found. Check the SD card.",
        ),
        (
            failure(Sample, OperationFailed, SamplePreview),
            "SAMPLE TROUBLE",
            "That sample couldn't be read. Check the SD card.",
        ),
        (
            failure(Sample, Unsupported, AudioCommand),
            "SAMPLE TROUBLE",
            "That sample is too long to play.",
        ),
        (
            failure(Recording, OperationFailed, RuntimeOperation::Recording),
            "CAN'T RECORD",
            "Recording failed. Check the SD card has space.",
        ),
        (
            failure(Runtime, Unavailable, SetupPortal),
            "SETUP PORTAL",
            "The setup portal hit a problem. Try again.",
        ),
        (
            failure(Runtime, OperationFailed, DeviceUpdate),
            "UPDATE",
            "The update didn't go through. Try again.",
        ),
        (
            failure(Runtime, OperationFailed, UserDataTransfer),
            "DATA TRANSFER",
            "The data transfer failed. Try again.",
        ),
        (
            failure(Runtime, OperationFailed, RuntimeDispatch),
            "OOPS",
            "Something went wrong. Try again.",
        ),
    ];
    for (error, title, text) in cases {
        assert_eq!(
            screen_of(&error, None),
            (title.to_owned(), text.to_owned()),
            "{error:?}"
        );
    }
}

#[test]
fn preset_save_failures_name_the_preset_and_the_real_cause() {
    let save = |message| {
        runtime_metadata(
            RuntimeErrorDomain::Storage,
            RuntimeErrorCode::OperationFailed,
            RuntimeOperation::StoreSavePreset,
            Some(message),
        )
    };
    assert_eq!(
        screen_of(&save("disk full"), Some("Pad Wash")),
        (
            "CAN'T SAVE".to_owned(),
            "Pad Wash was not saved. Check the SD card, then try again.".to_owned()
        )
    );
    assert_eq!(
        screen_of(
            &save("Save Pad Wash failed: preset already exists"),
            Some("Pad Wash")
        ),
        (
            "CAN'T SAVE".to_owned(),
            "Pad Wash was not saved. A preset with that name exists.".to_owned()
        )
    );
    assert_eq!(
        screen_of(&save("Preset catalog refresh failed: x"), Some("Pad Wash")),
        (
            "PRESET LIST".to_owned(),
            "Pad Wash was saved, but the preset list didn't refresh.".to_owned()
        )
    );
    assert_eq!(
        screen_of(&save("disk full"), Some("A very long preset name")).1,
        "A very long .. was not saved. Check the SD card, then try again."
    );
    assert!(screen_of(&save("disk full"), None)
        .1
        .starts_with("That preset was not saved."));
}

#[test]
fn text_beyond_four_rows_ends_the_last_row_with_an_ellipsis() {
    let rows = runtime_error_rows(&"abcdefghijklmnopqr ".repeat(6), Some("op"));
    assert_eq!(rows[3], "abcdefghijklmno...");
    assert_eq!(rows[4], "");
    assert_eq!(rows[5], "op");
}

#[test]
fn a_long_operation_id_is_clipped_to_the_row() {
    let rows = runtime_error_rows("x", Some("an_operation_id_far_too_long_to_fit"));
    assert_eq!(rows[ERROR_OPERATION_ROW], "an_operation_id...");
}
