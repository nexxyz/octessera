use super::{clip_display_line, RuntimeErrorCode, RuntimeSystemInfo, RuntimeSystemInfoError};

pub(super) const SYSTEM_INFO_LINE_WIDTH: usize = 18;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum NativeSystemInfoState {
    Loading,
    Ready(RuntimeSystemInfo),
    Unavailable(RuntimeSystemInfoError),
    Error(RuntimeSystemInfoError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct NativeSystemInfoModal {
    pub(super) state: NativeSystemInfoState,
    pub(super) scroll: usize,
}

impl NativeSystemInfoModal {
    pub(super) fn loading() -> Self {
        Self {
            state: NativeSystemInfoState::Loading,
            scroll: 0,
        }
    }

    pub(super) fn lines(&self) -> Vec<String> {
        let lines = match &self.state {
            NativeSystemInfoState::Loading => vec!["Loading info...".into()],
            NativeSystemInfoState::Ready(info) => vec![
                info_line("OS", &info.os),
                info_line("OS ver", &info.os_version),
                info_line("Octessera", &info.octessera_version),
                info_line("IP", info.primary_ip.as_deref().unwrap_or("unavailable")),
                info_line("MAC", info.primary_mac.as_deref().unwrap_or("unavailable")),
                info_line("Host", &info.hostname),
                info_line("Board", &info.board_profile),
            ],
            NativeSystemInfoState::Error(error) => vec![
                info_line("Error", &error.message),
                info_line("Code", &format_code(&error.code)),
            ],
            NativeSystemInfoState::Unavailable(error) => vec![
                info_line("Unavailable", &error.message),
                info_line("Code", &format_code(&error.code)),
            ],
        };
        lines
            .into_iter()
            .skip(self.scroll)
            .take(super::OLED_BODY_ROWS.saturating_sub(1))
            .collect()
    }

    pub(super) fn total_lines(&self) -> usize {
        match &self.state {
            NativeSystemInfoState::Loading => 1,
            NativeSystemInfoState::Ready(_) => 7,
            NativeSystemInfoState::Unavailable(_) | NativeSystemInfoState::Error(_) => 2,
        }
    }

    pub(super) fn turn(&mut self, delta: i8) {
        let max_scroll = self
            .total_lines()
            .saturating_sub(super::OLED_BODY_ROWS.saturating_sub(1));
        self.scroll =
            (self.scroll as isize + isize::from(delta)).clamp(0, max_scroll as isize) as usize;
    }
}

fn info_line(label: &str, value: &str) -> String {
    clip_display_line(&format!("{label}: {value}"), SYSTEM_INFO_LINE_WIDTH)
}

fn format_code(code: &RuntimeErrorCode) -> String {
    serde_json::to_string(code)
        .unwrap_or_else(|_| "error".into())
        .trim_matches('"')
        .into()
}
