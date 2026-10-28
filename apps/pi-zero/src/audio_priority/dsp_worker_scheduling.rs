use super::{configure_strict, syscalls};

pub(crate) const DSP_WORKER_CPUS: [usize; 2] = [2, 3];
pub(crate) const DSP_WORKER_PRIORITY: i32 = 70;
#[cfg(test)]
pub(crate) const ORANGE_WORKER_CPUS: [usize; 2] = DSP_WORKER_CPUS;
#[cfg(test)]
pub(crate) const ORANGE_WORKER_PRIORITY: i32 = DSP_WORKER_PRIORITY;

#[cfg(any(
    test,
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
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
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
pub(crate) fn pin_background_worker_to_dsp_cpus() {
    #[cfg(all(not(test), target_os = "linux"))]
    {
        let mut mask = unsafe { std::mem::zeroed::<libc::cpu_set_t>() };
        for cpu in DSP_WORKER_CPUS {
            unsafe { libc::CPU_SET(cpu, &mut mask) };
        }
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

// The MIDI sender sleeps until each timestamped message is due. It shares CPU0 with the
// runtime, away from the audio callback (CPU1) and the DSP workers (CPU2/3), and stays below
// the CPU0 mirror callback so a send never delays mirrored audio.
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    feature = "hardware-raspberry-pi-zero-2w"
))]
pub(crate) fn pin_midi_sender_thread() {
    #[cfg(not(test))]
    {
        const MIDI_SENDER_PRIORITY: i32 = 50;
        if let Err(failure) = configure_strict(super::PI_RUNTIME_CPU, MIDI_SENDER_PRIORITY) {
            eprintln!(
                "MIDI sender scheduling failed: {}",
                syscalls::format_failure("midi sender", failure)
            );
        }
    }
}
