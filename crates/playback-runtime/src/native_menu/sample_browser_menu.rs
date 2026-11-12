use super::{
    action_item, NativeMenuAction, NativeMenuItem, NativeMenuPick, NativeMenuValue,
    NativeSampleBrowserConfig, PlatformAction,
};

pub(super) fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    loaded: Option<&str>,
    sample_browser: Option<&NativeSampleBrowserConfig>,
    sample_favourite_dirs: &[String],
    sample_builtin_favourite_dirs: &[String],
) -> NativeMenuItem {
    let loaded = loaded.map(|path| path.replace('\\', "/"));
    let holds_loaded = |path: &str| {
        let path = path.replace('\\', "/");
        loaded.as_deref().is_some_and(|loaded| {
            loaded == path
                || loaded
                    .strip_prefix(path.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        })
    };
    let mut children = Vec::new();
    if let Some(browser) = sample_browser {
        if browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot {
            children.push(action_item(
                "..",
                format!("sample.up.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(PlatformAction::SampleUp {
                    instrument_slot,
                    sample_slot,
                }),
            ));
            for entry in &browser.entries {
                let (key, action) = if entry.is_dir {
                    (
                        "sample.enter",
                        PlatformAction::SampleEnter {
                            instrument_slot,
                            sample_slot,
                            dir: entry.path.clone(),
                        },
                    )
                } else {
                    (
                        "sample.pick",
                        PlatformAction::SamplePick {
                            instrument_slot,
                            sample_slot,
                            path: entry.path.clone(),
                        },
                    )
                };
                children.push(active_if(
                    holds_loaded(&entry.path),
                    action_item(
                        if entry.is_dir {
                            format!("[{}]", entry.name)
                        } else {
                            entry.name.clone()
                        },
                        format!("{key}.{instrument_slot}.{sample_slot}.{}", entry.path),
                        NativeMenuAction::PlatformEffect(action),
                    ),
                ));
            }
            if browser.dir.is_empty()
                && (!sample_builtin_favourite_dirs.is_empty() || !sample_favourite_dirs.is_empty())
            {
                for dir in
                    merged_favourite_dirs(sample_builtin_favourite_dirs, sample_favourite_dirs)
                {
                    children.push(active_if(
                        holds_loaded(&dir),
                        action_item(
                            format!("[★ {}]", favourite_label(&dir)),
                            format!("sample.favorite.open.{instrument_slot}.{sample_slot}.{dir}"),
                            NativeMenuAction::PlatformEffect(PlatformAction::SampleEnter {
                                instrument_slot,
                                sample_slot,
                                dir: dir.clone(),
                            }),
                        ),
                    ));
                }
                children.push(NativeMenuItem {
                    label: String::new(),
                    breadcrumb: None,
                    key: None,
                    value: NativeMenuValue::Group,
                    children: vec![],
                    pick: NativeMenuPick::None,
                });
            }
            if children.len() == 1 {
                children.push(action_item(
                    "(empty)",
                    format!("sample.open.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(PlatformAction::SampleOpen {
                        instrument_slot,
                        sample_slot,
                        dir: browser.dir.clone(),
                    }),
                ));
            }
            children.push(NativeMenuItem {
                label: String::new(),
                breadcrumb: None,
                key: None,
                value: NativeMenuValue::Group,
                children: vec![],
                pick: NativeMenuPick::None,
            });
            let is_builtin_favourite = sample_builtin_favourite_dirs
                .iter()
                .any(|dir| dir == &browser.dir);
            if is_builtin_favourite {
                children.push(NativeMenuItem {
                    label: "Built-in fav".into(),
                    breadcrumb: None,
                    key: None,
                    value: NativeMenuValue::Group,
                    children: vec![],
                    pick: NativeMenuPick::None,
                });
            } else {
                let (favorite_label, favorite_key, favorite_action) =
                    if sample_favourite_dirs.iter().any(|dir| dir == &browser.dir) {
                        (
                            "Remove Favorite",
                            "sample.favorite.remove",
                            PlatformAction::SampleFavoriteRemove {
                                instrument_slot,
                                sample_slot,
                            },
                        )
                    } else {
                        (
                            "Set Favorite",
                            "sample.favorite.set",
                            PlatformAction::SampleFavoriteSet {
                                instrument_slot,
                                sample_slot,
                            },
                        )
                    };
                children.push(action_item(
                    favorite_label,
                    format!("{favorite_key}.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(favorite_action),
                ));
            }
        } else {
            children.push(action_item(
                "(loading...)",
                format!("sample.open.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(PlatformAction::SampleOpen {
                    instrument_slot,
                    sample_slot,
                    dir: String::new(),
                }),
            ));
        }
    } else {
        children.push(action_item(
            "(loading...)",
            format!("sample.open.{instrument_slot}.{sample_slot}"),
            NativeMenuAction::PlatformEffect(PlatformAction::SampleOpen {
                instrument_slot,
                sample_slot,
                dir: String::new(),
            }),
        ));
    }
    NativeMenuItem {
        label: format!("S{} Browse", sample_slot + 1),
        breadcrumb: None,
        key: Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
        value: NativeMenuValue::Group,
        children,
        pick: NativeMenuPick::List,
    }
}

fn active_if(active: bool, item: NativeMenuItem) -> NativeMenuItem {
    NativeMenuItem {
        pick: if active {
            NativeMenuPick::Active
        } else {
            NativeMenuPick::None
        },
        ..item
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
