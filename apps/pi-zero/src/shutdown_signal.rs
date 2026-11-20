use std::sync::atomic::{AtomicBool, Ordering};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
pub(crate) fn install_signal_handlers() -> Result<(), String> {
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    unsafe {
        let handler = interrupt_handler as *const () as libc::sighandler_t;
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if libc::signal(signal, handler) == libc::SIG_ERR {
                return Err(format!(
                    "could not install shutdown handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(all(unix, board_orange))]
pub(crate) fn restore_default_handlers() {
    // SAFETY: SIG_DFL is always a valid disposition for these signals.
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            libc::signal(signal, libc::SIG_DFL);
        }
    }
}

#[cfg(all(not(unix), board_orange))]
pub(crate) fn restore_default_handlers() {}

#[cfg(not(unix))]
pub(crate) fn install_signal_handlers() -> Result<(), String> {
    Err("the runtime requires Unix signal handling".into())
}

pub(crate) fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

#[cfg(unix)]
extern "C" fn interrupt_handler(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}
