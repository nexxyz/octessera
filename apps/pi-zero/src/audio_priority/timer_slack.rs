/// Normal-priority threads let the kernel fire their timers up to 50 µs late so it can batch
/// wakeups. The runtime and UI threads wait on timers every few milliseconds, so the process
/// asks for 1 ns before it spawns them; new threads inherit the setting.
pub(crate) fn tighten_timer_slack() {
    if unsafe { libc::prctl(libc::PR_SET_TIMERSLACK, 1 as libc::c_ulong) } != 0 {
        eprintln!("Timer slack unchanged: {}", std::io::Error::last_os_error());
    }
}
