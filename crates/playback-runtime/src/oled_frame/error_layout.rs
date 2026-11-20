use super::model::OledRuntimeErrorMetadata;
use super::text_layout::{
    fit_line_ellipsis, force_line_ellipsis, wrap_text, RUNTIME_ERROR_BODY_RECT,
};

pub const ERROR_ROW_COUNT: usize = RUNTIME_ERROR_BODY_RECT.rows();
pub const ERROR_ROW_WIDTH: usize = RUNTIME_ERROR_BODY_RECT.columns();
pub const ERROR_TEXT_ROWS: usize = 4;
pub const ERROR_OPERATION_ROW: usize = ERROR_TEXT_ROWS + 1;
pub const PRESET_NAME_FALLBACK: &str = "That preset";
pub const PRESET_CATALOG_UNAVAILABLE: &str = "native preset catalog unavailable";
pub const PRESET_ALREADY_EXISTS: &str = "preset already exists";
pub const PRESET_CATALOG_REFRESH_FAILED: &str = "Preset catalog refresh failed";
const PRESET_NAME_COLUMNS: usize = 14;
const SD_RETRY: &str = "Check the SD card, then try again.";

pub struct ErrorScreen {
    pub title: &'static str,
    pub text: String,
}

pub fn runtime_error_screen(error: &OledRuntimeErrorMetadata) -> ErrorScreen {
    let code = error.code.as_deref().unwrap_or_default();
    let message = error.message.as_deref().unwrap_or_default();
    let (title, text) = match error.operation.as_deref().unwrap_or_default() {
        "store_save_preset" => preset_save_failure(message, error.preset_name.as_deref()),
        "store_save_default" | "store_save_recovery" => (
            "CAN'T SAVE",
            save_failure("Your patch was not saved.", code),
        ),
        "store_save_backup" => (
            "CAN'T SAVE",
            save_failure("A backup copy was not saved.", code),
        ),
        "store_save_system" => ("CAN'T SAVE", save_failure("Settings were not saved.", code)),
        "store_load_default" => (
            "CAN'T LOAD",
            "Your saved patch didn't load. The patch in use hasn't changed.".into(),
        ),
        "store_load_system" => (
            "CAN'T LOAD",
            "Settings didn't load. The settings in use haven't changed.".into(),
        ),
        "midi_message" => (
            "MIDI TROUBLE",
            "Couldn't send to the MIDI device. Check the cable.".into(),
        ),
        "midi_list_outputs" => (
            "MIDI TROUBLE",
            "Couldn't list the MIDI outputs. Try again.".into(),
        ),
        "audio_thread" => (
            "AUDIO TROUBLE",
            "Sound stopped. Check the audio output, then press Play.".into(),
        ),
        "audio_command" if error.domain.as_deref() != Some("sample") => (
            "AUDIO TROUBLE",
            "An audio command failed. Check the audio output.".into(),
        ),
        "recording" => (
            "CAN'T RECORD",
            "Recording failed. Check the SD card has space.".into(),
        ),
        "setup_portal" => (
            "SETUP PORTAL",
            "The setup portal hit a problem. Try again.".into(),
        ),
        "device_update" => ("UPDATE", "The update didn't go through. Try again.".into()),
        "user_data_transfer" => (
            "DATA TRANSFER",
            "The data transfer failed. Try again.".into(),
        ),
        _ if error.domain.as_deref() == Some("sample") => sample_trouble(error),
        _ => ("OOPS", "Something went wrong. Try again.".into()),
    };
    ErrorScreen { title, text }
}

fn save_failure(first: &str, code: &str) -> String {
    if code == "invalid_payload" {
        format!("{first} Its data wasn't valid.")
    } else {
        format!("{first} {SD_RETRY}")
    }
}

fn preset_save_failure(message: &str, preset_name: Option<&str>) -> (&'static str, String) {
    let name = preset_name.map_or_else(
        || PRESET_NAME_FALLBACK.to_owned(),
        |name| {
            if name.chars().count() > PRESET_NAME_COLUMNS {
                let kept: String = name.chars().take(PRESET_NAME_COLUMNS - 2).collect();
                format!("{}..", kept.trim_end())
            } else {
                name.to_owned()
            }
        },
    );
    if message.contains(PRESET_CATALOG_UNAVAILABLE)
        || message.contains(PRESET_CATALOG_REFRESH_FAILED)
    {
        (
            "PRESET LIST",
            format!("{name} was saved, but the preset list didn't refresh."),
        )
    } else if message.contains(PRESET_ALREADY_EXISTS) {
        (
            "CAN'T SAVE",
            format!("{name} was not saved. A preset with that name exists."),
        )
    } else {
        ("CAN'T SAVE", format!("{name} was not saved. {SD_RETRY}"))
    }
}

fn sample_trouble(error: &OledRuntimeErrorMetadata) -> (&'static str, String) {
    let text = match (error.operation.as_deref(), error.code.as_deref()) {
        (Some("sample_list"), _) => "Couldn't list the samples. Check the SD card.",
        (_, Some("not_found")) => "That sample wasn't found. Check the SD card.",
        (_, Some("unsupported")) => "That sample is too long to play.",
        _ => "That sample couldn't be read. Check the SD card.",
    };
    ("SAMPLE TROUBLE", text.into())
}

pub fn runtime_error_rows(text: &str, operation: Option<&str>) -> [String; ERROR_ROW_COUNT] {
    let mut rows = std::array::from_fn(|_| String::new());
    let wrapped = wrap_text(text, ERROR_ROW_WIDTH);
    let overflow = wrapped.len() > ERROR_TEXT_ROWS;
    for (index, row) in wrapped.into_iter().take(ERROR_TEXT_ROWS).enumerate() {
        rows[index] = if overflow && index + 1 == ERROR_TEXT_ROWS {
            force_line_ellipsis(&row, ERROR_ROW_WIDTH)
        } else {
            row
        };
    }
    rows[ERROR_OPERATION_ROW] = fit_line_ellipsis(operation.unwrap_or_default(), ERROR_ROW_WIDTH);
    rows
}
