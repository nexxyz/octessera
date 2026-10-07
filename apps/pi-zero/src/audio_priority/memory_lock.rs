/// Keeps the instrument's pages resident once touched. Without this, memory
/// pressure from other processes (for example the daily apt refresh) evicts
/// code the audio callback runs, and the callback stalls on SD card reads.
/// Both board services grant `LimitMEMLOCK=infinity` for this.
pub(crate) fn lock_process_memory() {
    let flags = libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT;
    if unsafe { libc::mlockall(flags) } != 0 {
        eprintln!(
            "Audio memory lock unavailable: {}",
            std::io::Error::last_os_error()
        );
    }
}
