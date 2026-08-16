use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

pub(crate) const STARTUP_ERROR_DIALOG_TITLE: &str = "Octessera startup error";

pub(crate) enum StartupDecision<T> {
    Continue(T),
    FailurePresented,
}

pub(crate) fn decide_startup<T, P>(
    result: Result<T, String>,
    present_error: P,
) -> StartupDecision<T>
where
    P: FnOnce(&str, &str),
{
    match result {
        Ok(value) => StartupDecision::Continue(value),
        Err(error) => {
            present_error(STARTUP_ERROR_DIALOG_TITLE, &error);
            StartupDecision::FailurePresented
        }
    }
}

pub(crate) fn present_native_startup_error(app: &tauri::App, title: &str, message: &str) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.hide() {
            eprintln!("{STARTUP_ERROR_DIALOG_TITLE}: unable to hide main window: {error}");
        }
    }

    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Error)
        .buttons(MessageDialogButtons::Ok)
        .show(|_| std::process::exit(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn startup_failure_presents_exact_error_without_starting_runtime() {
        let runtime_started = Cell::new(false);
        let mut presented = None;
        let decision = decide_startup(
            Err("unable to create store directory C:/data: access denied".to_string()),
            |title, message| presented = Some((title.to_string(), message.to_string())),
        );

        if matches!(&decision, StartupDecision::Continue(())) {
            runtime_started.set(true);
        }

        assert!(matches!(decision, StartupDecision::FailurePresented));
        assert_eq!(
            presented,
            Some((
                STARTUP_ERROR_DIALOG_TITLE.to_string(),
                "unable to create store directory C:/data: access denied".to_string()
            ))
        );
        assert!(!runtime_started.get());
    }

    #[test]
    fn successful_startup_continues_without_presenting_error() {
        let presented = Cell::new(false);
        let decision = decide_startup(Ok(7_u8), |_, _| presented.set(true));

        assert!(matches!(decision, StartupDecision::Continue(7)));
        assert!(!presented.get());
    }
}
