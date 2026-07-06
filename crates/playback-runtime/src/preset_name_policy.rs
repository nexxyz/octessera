use std::path::{Component, Path};

pub fn is_valid_preset_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name == name.trim()
        && !name.ends_with('.')
        && !name.chars().any(|ch| {
            ch.is_control() || matches!(ch, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        && Path::new(name)
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && !is_windows_reserved_name(name)
}

pub fn clean_preset_name(name: &str) -> String {
    let trimmed = name.trim();
    if is_valid_preset_name(trimmed) {
        trimmed.into()
    } else {
        fresh_preset_name()
    }
}

pub fn fresh_preset_name() -> String {
    use chrono::{Datelike, Local, Timelike};
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

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_name_policy_rejects_unsafe_names() {
        assert!(is_valid_preset_name("safe preset.1"));
        for name in [
            "../default",
            "presets/evil",
            r"C:\x",
            "",
            "   ",
            " bad",
            "bad ",
            "bad.",
            "bad/name",
            r"bad\name",
            "bad:name",
            "bad<name>",
            "bad\nname",
            "CON",
            "nul.json",
            "COM1",
            "LPT9.txt",
        ] {
            assert!(!is_valid_preset_name(name), "{name:?}");
        }
    }
}
