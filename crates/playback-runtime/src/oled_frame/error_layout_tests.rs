use super::error_layout::{runtime_error_rows, ERROR_ROW_COUNT, ERROR_ROW_WIDTH};
use super::font::glyph_rows;
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

#[test]
fn typed_producer_corpus_has_exact_bounded_supported_rows() {
    for (name, error) in producer_corpus() {
        let rows = runtime_error_rows(&error);
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
fn normal_error_rows_keep_fields_in_fixed_rows() {
    let rows = runtime_error_rows(&runtime_metadata(
        RuntimeErrorDomain::Sample,
        RuntimeErrorCode::NotFound,
        RuntimeOperation::AudioCommand,
        Some("sample not found"),
    ));
    assert_eq!(
        rows,
        [
            "DOMAIN sample".to_owned(),
            "CODE not found".to_owned(),
            "OP audio command".to_owned(),
            "MSG sample not".to_owned(),
            "    found".to_owned(),
            String::new(),
            String::new(),
        ]
    );
}

#[test]
fn long_underscored_unicode_path_preserves_path_cells() {
    let rows = runtime_error_rows(&metadata(
        Some("sample_device"),
        Some("not_found"),
        Some("sample_preview"),
        Some("/音色/very_long_file_abcdefghijklmnabcdefghijklmn"),
    ));
    assert_eq!(
        rows,
        [
            "DOMAIN sample d...".to_owned(),
            "CODE not found".to_owned(),
            "OP sample preview".to_owned(),
            "MSG /??/very_long_".to_owned(),
            "    file_abcdefghi".to_owned(),
            "    jklmnabcdefghi".to_owned(),
            "    jklmn".to_owned(),
        ]
    );
}

#[test]
fn missing_message_uses_visible_fallback_and_empty_continuations() {
    let rows = runtime_error_rows(&metadata(
        Some("audio"),
        Some("operation_failed"),
        Some("audio_command"),
        None,
    ));
    assert_eq!(rows[3], "MSG needs");
    assert_eq!(rows[4], "    attention");
    assert!(rows[5..].iter().all(String::is_empty));
}

#[test]
fn severe_message_truncation_ends_the_final_content_row() {
    let rows = runtime_error_rows(&metadata(
        Some("audio"),
        Some("operation_failed"),
        Some("audio_command"),
        Some("abcdefghijklmnabcdefghijklmnabcdefghijklmnabcdefghijklmnabcdefghijklmn"),
    ));
    assert_eq!(
        rows,
        [
            "DOMAIN audio".to_owned(),
            "CODE operation ...".to_owned(),
            "OP audio command".to_owned(),
            "MSG abcdefghijklmn".to_owned(),
            "    abcdefghijklmn".to_owned(),
            "    abcdefghijklmn".to_owned(),
            "    abcdefghijk...".to_owned(),
        ]
    );
}

#[test]
fn exact_fit_does_not_add_an_ellipsis() {
    let rows = runtime_error_rows(&metadata(
        Some("a"),
        Some("b"),
        Some("c"),
        Some("12345678901234123456789012341234567890123412345678901234"),
    ));
    assert_eq!(rows[3], "MSG 12345678901234");
    assert_eq!(rows[6], "    12345678901234");
    assert!(rows.iter().all(|row| !row.ends_with("...")));
}

#[test]
fn controls_whitespace_and_underscores_are_normalized_by_field_kind() {
    let rows = runtime_error_rows(&metadata(
        Some("audio__device\t"),
        Some("not_found"),
        Some("open\ndevice"),
        Some("bad\tpath\nwith\u{0000} controls_unchanged"),
    ));
    assert_eq!(rows[0], "DOMAIN audio de...");
    assert_eq!(rows[1], "CODE not found");
    assert_eq!(rows[2], "OP open device");
    assert_eq!(rows[3], "MSG bad path with");
    assert_eq!(rows[4], "    controls_uncha");
    assert_eq!(rows[5], "    nged");
    assert!(rows[6].is_empty());
}
