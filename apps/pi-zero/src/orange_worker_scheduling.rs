use super::{configure_strict, syscalls, ORANGE_WORKER_CPUS, ORANGE_WORKER_PRIORITY};

#[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
pub(crate) fn orange_worker_start_hook(parity: usize) -> Result<(), ()> {
    let Some(&cpu) = ORANGE_WORKER_CPUS.get(parity) else {
        return Err(());
    };
    match configure_strict(cpu, ORANGE_WORKER_PRIORITY) {
        Ok(_) => Ok(()),
        Err(failure) => {
            eprintln!(
                "Orange DSP worker parity={parity} scheduling failed: {}",
                syscalls::format_failure("worker", failure)
            );
            Err(())
        }
    }
}
