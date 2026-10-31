use super::super::{
    NativeConfirmDialog, NativeHelpPopup, NativeSetupPortalState, NativeSystemInfoModal,
    NativeUsbSdTransferModal, NativeUserDataRestoreState, NativeUserDataTransferState,
    RuntimeSetupPortalPhase, RuntimeUserDataRestorePhase, RuntimeUserDataTransferPhase,
    OLED_BODY_ROWS,
};
use super::{DisplayScrollMetadata, DisplaySnapshot};
use crate::oled_frame::OledDisplayLayout;
use crate::RuntimeSetupPortalErrorCode;

pub(super) fn user_data_restore_display(state: &NativeUserDataRestoreState) -> DisplaySnapshot {
    let (title, lines, selected_row) = match state.status.phase {
        RuntimeUserDataRestorePhase::Restoring => {
            ("Restoring...".into(), vec!["Please wait".into()], None)
        }
        RuntimeUserDataRestorePhase::Succeeded => (
            "Restore complete".into(),
            vec!["Data restored".into(), "> Close".into()],
            Some(1),
        ),
        RuntimeUserDataRestorePhase::Failed => (
            "Restore failed".into(),
            vec!["Pre-restore kept".into(), "> Close".into()],
            Some(1),
        ),
    };
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title,
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row,
    }
}

pub(super) fn confirm_dialog_display(confirm: &NativeConfirmDialog) -> DisplaySnapshot {
    let mut lines = confirm.lines.clone();
    for (index, option) in confirm.options.iter().enumerate() {
        let marker = if index == confirm.cursor { ">" } else { " " };
        lines.push(format!("{marker} {option}"));
    }
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: confirm.title.clone(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: Some(
            confirm
                .lines
                .len()
                .saturating_add(confirm.cursor)
                .min(OLED_BODY_ROWS.saturating_sub(1)),
        ),
    }
}

pub(super) fn setup_portal_display(state: &NativeSetupPortalState) -> DisplaySnapshot {
    let (title, mut lines, action) = match state.status.phase {
        RuntimeSetupPortalPhase::Starting => (
            "Wi-Fi Setup".to_string(),
            vec!["Starting hotspot...".into()],
            "Hide",
        ),
        RuntimeSetupPortalPhase::PortalReady => (
            "Wi-Fi Setup".to_string(),
            vec![
                "Hotspot:".into(),
                format!(
                    "Octessera Setup {}",
                    state.status.portal_suffix.as_deref().unwrap_or("----")
                ),
                "Open 192.168.42.1".into(),
                "Portal: 10 minutes".into(),
            ],
            "Hide",
        ),
        RuntimeSetupPortalPhase::Finalizing => (
            "Wi-Fi Setup".to_string(),
            vec!["Applying settings...".into()],
            "Hide",
        ),
        RuntimeSetupPortalPhase::Succeeded => (
            "Wi-Fi Setup".to_string(),
            vec![
                "Setup complete".into(),
                "IP in Maintenance > Sys. Info".into(),
                "No reboot needed".into(),
            ],
            "Close",
        ),
        RuntimeSetupPortalPhase::Failed
            if state.status.error_code
                == Some(RuntimeSetupPortalErrorCode::RadioRestartRequired) =>
        {
            (
                "Wi-Fi Setup".to_string(),
                vec!["Wi-Fi chip stopped".into(), "Restart the device".into()],
                "Close",
            )
        }
        RuntimeSetupPortalPhase::Failed => (
            "Wi-Fi Setup".to_string(),
            vec!["Setup failed".into(), "Check the device status".into()],
            "Close",
        ),
        RuntimeSetupPortalPhase::TimedOut => (
            "Wi-Fi Setup".to_string(),
            vec!["Setup timed out".into(), "Portal closed".into()],
            "Close",
        ),
        RuntimeSetupPortalPhase::Unsupported => (
            "Wi-Fi Setup".to_string(),
            vec!["Not available on".into(), "desktop".into()],
            "Close",
        ),
    };
    lines.push(format!("> {action}"));
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Card,
        title,
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: Some(line_count - 1),
    }
}

pub(super) fn user_data_transfer_display(state: &NativeUserDataTransferState) -> DisplaySnapshot {
    let (title, lines) = match state.status.phase {
        RuntimeUserDataTransferPhase::Ready => {
            let url = state.status.url.as_deref().unwrap_or_default();
            let (host, port) = transfer_url_host_port(url);
            let expires = state
                .status
                .expires_in_seconds
                .map(|seconds| u32::from(seconds).div_ceil(60))
                .unwrap_or_default();
            (
                "Backup / Restore".into(),
                vec![
                    "Use local client".into(),
                    format!("IP {host}"),
                    format!("PORT {port}"),
                    format!("CODE {}", state.status.code.as_deref().unwrap_or_default()),
                    format!("Ends in {expires} min"),
                    "> Stop service".into(),
                ],
            )
        }
        RuntimeUserDataTransferPhase::Unsupported => (
            "Backup / Restore".into(),
            vec![
                "Not supported here".into(),
                "Use a Pi device".into(),
                "> Close".into(),
            ],
        ),
        RuntimeUserDataTransferPhase::Closed => unreachable!("closed transfer is cleared"),
    };
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Card,
        title,
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: Some(line_count.saturating_sub(1)),
    }
}

fn transfer_url_host_port(url: &str) -> (&str, &str) {
    let authority = url
        .strip_prefix("http://")
        .unwrap_or_default()
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    if let Some((host, port)) = authority.rsplit_once(':') {
        (host.trim_start_matches('[').trim_end_matches(']'), port)
    } else {
        (authority, "80")
    }
}

pub(super) fn usb_sd_transfer_modal_display(modal: &NativeUsbSdTransferModal) -> DisplaySnapshot {
    let mut lines = modal.lines.clone();
    lines.push("> Stop Transfer".into());
    lines.truncate(OLED_BODY_ROWS);
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: modal.title.clone(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: Some(line_count.saturating_sub(1)),
    }
}

pub(super) fn system_info_modal_display(modal: &NativeSystemInfoModal) -> DisplaySnapshot {
    let mut lines = modal.lines();
    lines.push("> Back".into());
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: "Sys. Info".into(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: Some(DisplayScrollMetadata {
            scroll_offset: modal.scroll,
            total_rows: modal.total_lines() + 1,
            visible_rows: line_count,
        }),
        selected_row: Some(line_count.saturating_sub(1)),
    }
}

pub(super) fn help_popup_display(help: &NativeHelpPopup) -> DisplaySnapshot {
    let mut lines = help
        .lines
        .iter()
        .skip(help.scroll)
        .take(OLED_BODY_ROWS - 1)
        .cloned()
        .collect::<Vec<_>>();
    lines.push("> Close".into());
    let line_count = lines.len();
    DisplaySnapshot {
        body_layout: OledDisplayLayout::Rows,
        title: help.title.clone(),
        lines,
        colors: vec![platform_core::palette::WHITE_RGB565; line_count],
        bar_values: vec![None; line_count],
        full_lines: vec![None; line_count],
        rows: vec![None; line_count],
        scroll: None,
        selected_row: Some(
            help.lines
                .len()
                .saturating_sub(help.scroll)
                .min(OLED_BODY_ROWS - 1),
        ),
    }
}
