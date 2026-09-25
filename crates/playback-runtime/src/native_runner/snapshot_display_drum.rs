use super::{menu_display, overlay_display, DisplaySnapshot};
use crate::native_menu::NativeMenuSnapshot;
use serde_json::Value;

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
        display.bar_values.push(Value::Null);
        display.full_lines.push(None);
        if let Some(scroll) = &mut display.scroll {
            scroll.total_rows += 1;
            scroll.visible_rows = display.lines.len();
        }
    }
    display
}

pub(super) fn cell_tune_display(runner: &NativeRunner) -> Option<DisplaySnapshot> {
    let (slot, selected) = runner.drum_cell_tune?;
    let lines = if let Some((x, y)) = selected {
        let cell = runner
            .instruments
            .get(slot)?
            .drum_config
            .get("assignments")?
            .as_array()?
            .iter()
            .find(|cell| cell["x"] == x && cell["y"] == y)?;
        let voice = cell.get("voice")?.as_u64()? as usize;
        let sound = runner.instruments[slot].drum_config["voices"][voice]["sound"].as_str()?;
        let name = match sound {
            "kick" => "Kick",
            "snare" => "Snare",
            "closed_hat" => "Closed Hat",
            "open_hat" => "Open Hat",
            "low_tom" => "Low Tom",
            "high_tom" => "High Tom",
            "clap" => "Clap",
            "rim" => "Rim",
            _ => sound,
        };
        let tune = cell.get("tuneSemis").and_then(Value::as_i64).unwrap_or(0);
        vec![format!("{name} ({x},{y})"), format!("Tune {tune:+} st")]
    } else {
        vec!["Choose drum cell".into()]
    };
    Some(overlay_display("Cell Tune".into(), lines))
}
