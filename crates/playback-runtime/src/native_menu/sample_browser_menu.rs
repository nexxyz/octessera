use super::{
    action_item, NativeMenuAction, NativeMenuItem, NativeMenuValue, NativeSampleBrowserConfig,
};

pub(super) fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    sample_browser: Option<&NativeSampleBrowserConfig>,
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
