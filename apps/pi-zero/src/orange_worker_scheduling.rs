use super::{configure_strict, syscalls, DSP_WORKER_CPUS, DSP_WORKER_PRIORITY};

pub(crate) fn benchmark_worker_start_hook(parity: usize) -> Result<(), ()> {
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

#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn orange_worker_start_hook(parity: usize) -> Result<(), ()> {
    benchmark_worker_start_hook(parity)
}
