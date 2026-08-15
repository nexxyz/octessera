use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
pub(crate) fn install_signal_handlers() -> Result<(), String> {
    unsafe {
        let handler = interrupt_handler as *const () as libc::sighandler_t;
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if libc::signal(signal, handler) == libc::SIG_ERR {
                return Err(format!(
                    "could not install Orange shutdown handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(not(unix))]
pub(crate) fn install_signal_handlers() -> Result<(), String> {
    Err("Orange foreground candidate requires Unix signal handling".into())
}

pub(crate) fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

#[cfg(unix)]
extern "C" fn interrupt_handler(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}
