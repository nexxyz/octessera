use chrono::{Datelike, Local, Timelike};

pub(super) fn clean_preset_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        fresh_preset_name()
    } else {
        trimmed.into()
    }
}

pub(super) fn fresh_preset_name() -> String {
    let now = Local::now();
    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}
