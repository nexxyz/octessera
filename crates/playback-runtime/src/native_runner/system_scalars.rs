/// One top-level System setting saved as a single value: its saved key, the saved and menu
/// range of a number setting (`None` for switches and choices), and whether an Aux side may
/// turn it. Every one is required in a full config, kept by user-data preferences and
/// stripped from a portable patch.
pub(crate) struct SystemScalar {
    pub(crate) key: &'static str,
    pub(crate) range: Option<(i32, i32)>,
    pub(crate) aux_turn: bool,
}

pub(crate) const MASTER_VOLUME_PCT: (i32, i32) = (0, 100);
pub(crate) const SCREEN_TIMER_SECONDS: (i32, i32) = (0, 600);
pub(crate) const DISPLAY_BRIGHTNESS: (i32, i32) = (1, 16);
pub(crate) const LED_BRIGHTNESS_PCT: (i32, i32) = (10, 100);

const fn setting(key: &'static str) -> SystemScalar {
    SystemScalar {
        key,
        range: None,
        aux_turn: true,
    }
}

const fn number(key: &'static str, range: (i32, i32)) -> SystemScalar {
    SystemScalar {
        key,
        range: Some(range),
        aux_turn: true,
    }
}

const fn fixed(key: &'static str) -> SystemScalar {
    SystemScalar {
        key,
        range: None,
        aux_turn: false,
    }
}

pub(crate) const SYSTEM_SCALARS: &[SystemScalar] = &[
    number("masterVolume", MASTER_VOLUME_PCT),
    setting("ghostTiles"),
    setting("auxAutoHints"),
    setting("auxManualHints"),
    setting("inputEventsWhilePaused"),
    setting("numericDisplayMode"),
    setting("mainFont"),
    setting("smallFont"),
    setting("homeVisual"),
    setting("perfViewOnBack"),
    setting("perfViewAtBoot"),
    number("dimTimerSeconds", SCREEN_TIMER_SECONDS),
    number("screenSleepSeconds", SCREEN_TIMER_SECONDS),
    number("displayBrightness", DISPLAY_BRIGHTNESS),
    number("gridBrightness", LED_BRIGHTNESS_PCT),
    number("buttonBrightness", LED_BRIGHTNESS_PCT),
    setting("autoSaveDefault"),
    setting("rollingBackups"),
    fixed("auxAutoMapEnabled"),
    fixed("auxPriority"),
];

pub(crate) fn system_scalar(key: &str) -> Option<&'static SystemScalar> {
    SYSTEM_SCALARS.iter().find(|scalar| scalar.key == key)
}

pub(crate) fn system_scalar_keys() -> impl Iterator<Item = &'static str> {
    SYSTEM_SCALARS.iter().map(|scalar| scalar.key)
}
