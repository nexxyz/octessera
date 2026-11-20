use super::{configure_strict, syscalls};

pub(crate) const DSP_WORKER_CPUS: [usize; 2] = [2, 3];
pub(crate) const DSP_WORKER_PRIORITY: i32 = 70;
#[cfg(test)]
pub(crate) const ORANGE_WORKER_CPUS: [usize; 2] = DSP_WORKER_CPUS;
#[cfg(test)]
pub(crate) const ORANGE_WORKER_PRIORITY: i32 = DSP_WORKER_PRIORITY;

#[cfg(any(test, board_orange, board_raspberry))]
pub(crate) fn pi_worker_start_hook(parity: usize) -> Result<(), ()> {
    let Some(&cpu) = DSP_WORKER_CPUS.get(parity) else {
        return Err(());
    };
    match configure_strict(cpu, DSP_WORKER_PRIORITY) {
        Ok(_) => Ok(()),
        Err(failure) => {
            eprintln!(
                "DSP worker parity={parity} scheduling failed: {}",
                syscalls::format_failure("worker", failure)
            );
            Err(())
        }
    }
}

// Background work (bulk sample decoding, the Bluetooth audio feeder) on CPU0
// delays the audio interrupt's wakeup of the callback; keep it on the DSP
// cores, where the realtime workers preempt it.
#[cfg(not(board_host))]
pub(crate) fn pin_background_worker_to_dsp_cpus() {
    #[cfg(all(not(test), target_os = "linux"))]
    {
        // SAFETY: cpu_set_t is a plain bit array, so all-zero bytes are a valid empty set.
        let mut mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        for cpu in DSP_WORKER_CPUS {
            // SAFETY: CPU_SET writes one bit of the owned local mask and bounds-checks `cpu`.
            unsafe { libc::CPU_SET(cpu, &mut mask) };
        }
        // SAFETY: the calling thread is live and the size matches the borrowed local mask.
        let result = unsafe {
            libc::pthread_setaffinity_np(
                libc::pthread_self(),
                std::mem::size_of::<libc::cpu_set_t>(),
                &mask,
            )
        };
        if result != 0 {
            eprintln!(
                "background worker affinity failed: {}",
                std::io::Error::from_raw_os_error(result)
            );
        }
    }
}

// The MIDI sender sleeps until each timestamped message is due, and the MIDI input thread
// stamps each clock byte on arrival. Both share CPU0 with the runtime, away from the audio
// callback (CPU1) and the DSP workers (CPU2/3), and stay below the CPU0 mirror callback so
// MIDI never delays mirrored audio.
#[cfg(not(board_host))]
pub(crate) fn pin_midi_thread(_role: &str) {
    #[cfg(not(test))]
    {
        const MIDI_THREAD_PRIORITY: i32 = 50;
        if let Err(failure) = configure_strict(super::PI_RUNTIME_CPU, MIDI_THREAD_PRIORITY) {
            eprintln!(
                "{_role} scheduling failed: {}",
                syscalls::format_failure(_role, failure)
            );
        }
    }
}

// The signal analysis worker shares CPU0 with the runtime and the render worker. SCHED_IDLE lets
// either preempt it the moment they wake, so a slow picture only lowers its update rate.
#[cfg(not(board_host))]
pub(crate) fn idle_signal_analysis_thread() {
    #[cfg(all(not(test), target_os = "linux"))]
    {
        let param = libc::sched_param { sched_priority: 0 };
        // SAFETY: pid 0 names the calling thread and `param` outlives the call.
        if unsafe { libc::sched_setscheduler(0, libc::SCHED_IDLE, &param) } != 0 {
            eprintln!(
                "signal analysis worker SCHED_IDLE failed: {}",
                std::io::Error::last_os_error()
            );
        }
    }
}
