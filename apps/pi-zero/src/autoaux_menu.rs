use crate::input::{encoder_press_message, encoder_turn_message, neokey_message};
use crate::normal_menu::is_normal_menu_snapshot;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime, RuntimeTransportState};

pub(super) type DispatchInput<'a> =
    dyn FnMut(&mut PlaybackRuntime, &mut NativeRunner, HostMessage) -> Result<(), String> + 'a;

pub(super) fn navigate_to_cutoff(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    dispatch: &mut DispatchInput<'_>,
    board: &str,
) -> Result<(), String> {
    expect_focus(playback, "MENU", "Build", board)?;
    turn_main(playback, runner, dispatch, 2)?;
    expect_focus(playback, "MENU", "Shape", board)?;
    press_main(playback, runner, dispatch)?;
    expect_focus(playback, "Shape", "Instruments", board)?;
    press_main(playback, runner, dispatch)?;
    expect_focus(playback, "Instruments", "I1: synth", board)?;
    press_main(playback, runner, dispatch)?;
    expect_focus(playback, "synth", "Type synth", board)?;
    turn_main(playback, runner, dispatch, 2)?;
    expect_focus(playback, "synth", "Synth", board)?;
    press_main(playback, runner, dispatch)?;
    expect_focus(playback, "Synth", "Preset", board)?;
    turn_main(playback, runner, dispatch, 3)?;
    expect_focus(playback, "Synth", "Filter", board)?;
    press_main(playback, runner, dispatch)?;
    expect_focus(playback, "Filter", "Type", board)?;
    turn_main(playback, runner, dispatch, 1)?;
    cutoff_display_value(playback, board)?;
    Ok(())
}

pub(super) fn enable_study_auto_save(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    dispatch: &mut DispatchInput<'_>,
    board: &str,
) -> Result<(), String> {
    expect_focus(playback, "MENU", "Build", board)?;
    turn_main(playback, runner, dispatch, 5)?;
    press_main(playback, runner, dispatch)?;
    move_to_selected_row(playback, runner, dispatch, "Saves", 24, board)?;
    press_main(playback, runner, dispatch)?;
    move_to_selected_row(playback, runner, dispatch, "Default", 4, board)?;
    press_main(playback, runner, dispatch)?;
    let (_, line) = selected_line(playback, board)?;
    if !line.contains("Auto Save") {
        return Err(format!("{board} Auto Save row is unavailable: {line:?}"));
    }
    if line.ends_with(" Off") {
        press_main(playback, runner, dispatch)?;
        turn_main(playback, runner, dispatch, 1)?;
        press_main(playback, runner, dispatch)?;
    } else if !line.ends_with(" On") {
        return Err(format!(
            "{board} Auto Save row has an unknown value: {line:?}"
        ));
    }
    expect_focus(playback, "Default", "Auto Save On", board)?;
    for _ in 0..3 {
        for pressed in [true, false] {
            dispatch(
                playback,
                runner,
                neokey_message(0, pressed)
                    .ok_or_else(|| format!("{board} Back NeoKey input unavailable"))?,
            )?;
        }
    }
    turn_main(playback, runner, dispatch, -5)?;
    expect_focus(playback, "MENU", "Build", board)?;
    Ok(())
}

pub(super) fn cutoff_display_value(playback: &PlaybackRuntime, board: &str) -> Result<u16, String> {
    let (_, line) = selected_line(playback, board)?;
    if !line.contains("Cutoff") {
        return Err(format!(
            "{board} Aux timing smoke lost Cutoff focus: {line:?}"
        ));
    }
    line.split_whitespace()
        .last()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|value| *value <= 255)
        .ok_or_else(|| format!("{board} Cutoff row has no numeric display value: {line:?}"))
}

pub(super) fn require_stopped_normal_menu(
    playback: &PlaybackRuntime,
    board: &str,
) -> Result<(), String> {
    if !playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Stopped)
    {
        return Err(format!(
            "{board} Aux timing smoke menu verification requires Stopped transport"
        ));
    }
    let snapshot = playback
        .last_snapshot()
        .ok_or_else(|| format!("{board} Aux timing smoke has no menu snapshot"))?;
    if !is_normal_menu_snapshot(snapshot) {
        return Err(format!(
            "{board} Aux timing smoke requires a normal menu snapshot"
        ));
    }
    Ok(())
}

fn move_to_selected_row(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    dispatch: &mut DispatchInput<'_>,
    label: &str,
    max_turns: usize,
    board: &str,
) -> Result<(), String> {
    for _ in 0..max_turns {
        if selected_line(playback, board)?.1.contains(label) {
            return Ok(());
        }
        turn_main(playback, runner, dispatch, 1)?;
    }
    Err(format!("{board} timing setup could not focus {label}"))
}

fn selected_line(playback: &PlaybackRuntime, board: &str) -> Result<(String, String), String> {
    require_stopped_normal_menu(playback, board)?;
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
    board: &str,
) -> Result<(), String> {
    let (title, line) = selected_line(playback, board)?;
    if !title.contains(title_suffix) || !line.contains(row_fragment) {
        return Err(format!(
            "{board} Aux timing smoke focus mismatch: expected {title_suffix}/{row_fragment}, got {title:?}/{line:?}"
        ));
    }
    Ok(())
}

fn turn_main(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    dispatch: &mut DispatchInput<'_>,
    delta: i8,
) -> Result<(), String> {
    dispatch(
        playback,
        runner,
        encoder_turn_message("encoder_main", delta),
    )
}

fn press_main(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    dispatch: &mut DispatchInput<'_>,
) -> Result<(), String> {
    dispatch(playback, runner, encoder_press_message("encoder_main"))
}
