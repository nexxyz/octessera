use super::{menu_display, overlay_display, DisplaySnapshot};
use crate::native_menu::NativeMenuSnapshot;

use super::super::NativeRunner;

pub(super) fn play_menu_display(
    runner: &NativeRunner,
    menu: NativeMenuSnapshot,
) -> DisplaySnapshot {
    if menu.path == "/P/No Drum slot"
        && !runner
            .instruments
            .iter()
            .any(|instrument| instrument.kind == "drum")
    {
        return overlay_display("No Drum slot".into(), vec!["No Drum slot".into()]);
    }
    let show_guidance = menu.path == "/P/Drums"
        && menu
            .line_keys
            .iter()
            .any(|key| key.as_deref() == Some("play.drums.slot"))
        && runner
            .instruments
            .iter()
            .any(|instrument| instrument.kind == "drum");
    let mut display = menu_display(runner, menu);
    if show_guidance {
        display.lines.push("Grid: tap to play".into());
        display.colors.push(platform_core::palette::WHITE_RGB565);
        display.bar_values.push(None);
        display.full_lines.push(None);
        display.rows.push(None);
        if let Some(scroll) = &mut display.scroll {
            scroll.total_rows += 1;
            scroll.visible_rows = display.lines.len();
        }
    }
    display
}
