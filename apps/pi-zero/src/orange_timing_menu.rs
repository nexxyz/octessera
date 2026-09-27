use super::dispatch;
use crate::input::{encoder_press_message, encoder_turn_message, neokey_message};
use crate::normal_menu::is_normal_menu_snapshot;
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{NativeRunner, PlaybackRuntime, RuntimeTransportState};

pub(super) fn navigate_to_cutoff(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    expect_focus(playback, "MENU", "Build")?;
    turn_main(playback, runner, host, 2)?;
    expect_focus(playback, "MENU", "Shape")?;
    press_main(playback, runner, host)?;
    expect_focus(playback, "Shape", "Instruments")?;
    press_main(playback, runner, host)?;
    expect_focus(playback, "Instruments", "I1: synth")?;
    press_main(playback, runner, host)?;
    expect_focus(playback, "synth", "Type synth")?;
    turn_main(playback, runner, host, 2)?;
    expect_focus(playback, "synth", "Synth")?;
    press_main(playback, runner, host)?;
    expect_focus(playback, "Synth", "Preset")?;
    turn_main(playback, runner, host, 3)?;
    expect_focus(playback, "Synth", "Filter")?;
    press_main(playback, runner, host)?;
    expect_focus(playback, "Filter", "Type")?;
    turn_main(playback, runner, host, 1)?;
    cutoff_display_value(playback)?;
    Ok(())
}

pub(super) fn enable_study_auto_save(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    expect_focus(playback, "MENU", "Build")?;
    turn_main(playback, runner, host, 5)?;
    press_main(playback, runner, host)?;
    move_to_selected_row(playback, runner, host, "Saves", 24)?;
    press_main(playback, runner, host)?;
    move_to_selected_row(playback, runner, host, "Default", 4)?;
    press_main(playback, runner, host)?;
    let (_, line) = selected_line(playback)?;
    if !line.contains("Auto Save") {
        return Err(format!("Orange Auto Save row is unavailable: {line:?}"));
    }
    if line.ends_with(" Off") {
        press_main(playback, runner, host)?;
        turn_main(playback, runner, host, 1)?;
        press_main(playback, runner, host)?;
    } else if !line.ends_with(" On") {
        return Err(format!(
            "Orange Auto Save row has an unknown value: {line:?}"
        ));
    }
    expect_focus(playback, "Default", "Auto Save On")?;
    for _ in 0..3 {
        for pressed in [true, false] {
            dispatch(
                playback,
                runner,
                host,
                neokey_message(0, pressed).ok_or("Orange Back NeoKey input unavailable")?,
            )?;
        }
    }
    turn_main(playback, runner, host, -5)?;
    expect_focus(playback, "MENU", "Build")?;
    Ok(())
}

fn move_to_selected_row(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    label: &str,
    max_turns: usize,
) -> Result<(), String> {
    for _ in 0..max_turns {
        if selected_line(playback)?.1.contains(label) {
            return Ok(());
        }
        turn_main(playback, runner, host, 1)?;
    }
    Err(format!("Orange timing setup could not focus {label}"))
}

fn selected_line(playback: &PlaybackRuntime) -> Result<(String, String), String> {
    require_stopped_normal_menu(playback)?;
    let snapshot = playback.last_snapshot().expect("menu snapshot checked");
    let display = snapshot.get("display").ok_or("menu display is missing")?;
    let title = display
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let row = snapshot
        .get("selectedRow")
        .or_else(|| display.get("selectedRow"))
        .and_then(serde_json::Value::as_u64)
        .and_then(|row| usize::try_from(row).ok())
        .ok_or("menu selected row is missing")?;
    display
        .get("lines")
        .and_then(serde_json::Value::as_array)
        .and_then(|lines| lines.get(row))
        .and_then(serde_json::Value::as_str)
        .map(|line| (title, line.to_owned()))
        .ok_or("menu selected line is missing".into())
}

fn expect_focus(
    playback: &PlaybackRuntime,
    title_suffix: &str,
    row_fragment: &str,
) -> Result<(), String> {
    let (title, line) = selected_line(playback)?;
    if !title.contains(title_suffix) || !line.contains(row_fragment) {
        return Err(format!(
            "Orange Aux timing smoke focus mismatch: expected {title_suffix}/{row_fragment}, got {title:?}/{line:?}"
        ));
    }
    Ok(())
}

pub(super) fn cutoff_display_value(playback: &PlaybackRuntime) -> Result<u16, String> {
    let (_, line) = selected_line(playback)?;
    if !line.contains("Cutoff") {
        return Err(format!(
            "Orange Aux timing smoke lost Cutoff focus: {line:?}"
        ));
    }
    line.split_whitespace()
        .last()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value <= 255)
        .ok_or_else(|| format!("Orange Cutoff row has no numeric display value: {line:?}"))
}

pub(super) fn require_stopped_normal_menu(playback: &PlaybackRuntime) -> Result<(), String> {
    if !playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Stopped)
    {
        return Err("Orange Aux timing smoke menu verification requires Stopped transport".into());
    }
    let snapshot = playback
        .last_snapshot()
        .ok_or("Orange Aux timing smoke has no menu snapshot")?;
    if !is_normal_menu_snapshot(snapshot) {
        return Err("Orange Aux timing smoke requires a normal menu snapshot".into());
    }
    Ok(())
}

fn turn_main(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    delta: i8,
) -> Result<(), String> {
    dispatch(
        playback,
        runner,
        host,
        encoder_turn_message("encoder_main", delta),
    )
}

fn press_main(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    dispatch(
        playback,
        runner,
        host,
        encoder_press_message("encoder_main"),
    )
}
