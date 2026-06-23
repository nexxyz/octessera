use super::{
    action_item, NativeMenuAction, NativeMenuItem, NativeMenuValue, NativeSampleBrowserConfig,
};

pub(super) fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    sample_browser: Option<&NativeSampleBrowserConfig>,
    sample_favourite_dirs: &[String],
    sample_builtin_favourite_dirs: &[String],
) -> NativeMenuItem {
    let mut children = Vec::new();
    if let Some(browser) = sample_browser {
        if browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot {
            children.push(action_item(
                "..",
                format!("sample.up.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.up:{instrument_slot}:{sample_slot}"
                )),
            ));
            for entry in &browser.entries {
                let action = if entry.is_dir {
                    "sample.enter"
                } else {
                    "sample.pick"
                };
                children.push(action_item(
                    if entry.is_dir {
                        format!("[{}]", entry.name)
                    } else {
                        entry.name.clone()
                    },
                    format!("{action}.{instrument_slot}.{sample_slot}.{}", entry.path),
                    NativeMenuAction::PlatformEffect(format!(
                        "{action}:{instrument_slot}:{sample_slot}:{}",
                        entry.path
                    )),
                ));
            }
            if browser.dir.is_empty()
                && (!sample_builtin_favourite_dirs.is_empty() || !sample_favourite_dirs.is_empty())
            {
                for dir in
                    merged_favourite_dirs(sample_builtin_favourite_dirs, sample_favourite_dirs)
                {
                    children.push(action_item(
                        format!("[★ {}]", favourite_label(&dir)),
                        format!("sample.favorite.open.{instrument_slot}.{sample_slot}.{dir}"),
                        NativeMenuAction::PlatformEffect(format!(
                            "sample.enter:{instrument_slot}:{sample_slot}:{dir}"
                        )),
                    ));
                }
                children.push(NativeMenuItem {
                    label: String::new(),
                    key: None,
                    value: NativeMenuValue::Group,
                    children: vec![],
                });
            }
            if children.len() == 1 {
                children.push(action_item(
                    "(empty)",
                    format!("sample.open.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(format!(
                        "sample.open:{instrument_slot}:{sample_slot}:{}",
                        browser.dir
                    )),
                ));
            }
            children.push(NativeMenuItem {
                label: String::new(),
                key: None,
                value: NativeMenuValue::Group,
                children: vec![],
            });
            let is_builtin_favourite = sample_builtin_favourite_dirs
                .iter()
                .any(|dir| dir == &browser.dir);
            if is_builtin_favourite {
                children.push(NativeMenuItem {
                    label: "Built-in favourite".into(),
                    key: None,
                    value: NativeMenuValue::Group,
                    children: vec![],
                });
            } else {
                let favorite_action = if sample_favourite_dirs.iter().any(|dir| dir == &browser.dir)
                {
                    "sample.favorite.remove"
                } else {
                    "sample.favorite.set"
                };
                let favorite_label = if favorite_action.ends_with("remove") {
                    "Remove favourite"
                } else {
                    "Set favourite"
                };
                children.push(action_item(
                    favorite_label,
                    format!("{favorite_action}.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(format!(
                        "{favorite_action}:{instrument_slot}:{sample_slot}"
                    )),
                ));
            }
        } else {
            children.push(action_item(
                "(loading...)",
                format!("sample.open.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.open:{instrument_slot}:{sample_slot}:"
                )),
            ));
        }
    } else {
        children.push(action_item(
            "(loading...)",
            format!("sample.open.{instrument_slot}.{sample_slot}"),
            NativeMenuAction::PlatformEffect(format!(
                "sample.open:{instrument_slot}:{sample_slot}:"
            )),
        ));
    }
    NativeMenuItem {
        label: format!("S{} Browse", sample_slot + 1),
        key: Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
        value: NativeMenuValue::Group,
        children,
    }
}

fn merged_favourite_dirs(builtins: &[String], user: &[String]) -> Vec<String> {
    let mut dirs = Vec::new();
    for dir in builtins.iter().chain(user) {
        if !dirs.iter().any(|existing| existing == dir) {
            dirs.push(dir.clone());
        }
    }
    dirs
}

fn favourite_label(dir: &str) -> String {
    match dir {
        "" => "Samples".into(),
        "sd-card" => "SD card".into(),
        "userdata" => "User data".into(),
        _ => dir.rsplit('/').next().unwrap_or(dir).to_string(),
    }
}
