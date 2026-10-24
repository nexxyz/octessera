//! Diagnostic run modes: the DSP profile and timing probes exist only in the instrumented
//! `profiling` build. Production builds refuse their flags instead of starting up.

#[cfg(all(feature = "profiling", feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn run_requested() {
    use crate::{dsp_profile, exit_code};
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
}

#[cfg(all(feature = "profiling", not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn run_requested() {
    use crate::{dsp_profile, exit_code, timing_probe};
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if timing_probe::requested() {
        std::process::exit(exit_code(timing_probe::run()));
    }
}

#[cfg(not(feature = "profiling"))]
pub(crate) fn run_requested() {
    let flagged = std::env::args()
        .skip(1)
        .any(|arg| matches!(arg.as_str(), "--profile-dsp" | "--timing-probe"));
    let dsp_profile = cfg!(not(feature = "hardware-orange-pi-zero-2w"))
        && std::env::var("OCTESSERA_PI_PROFILE_DSP").is_ok_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "profile" | "dsp"
            )
        });
    let timing_probe = std::env::var("OCTESSERA_PI_TIMING_PROBE").as_deref() == Ok("1");
    if flagged || dsp_profile || timing_probe {
        eprintln!("profiling modes need the profiling build (cargo feature `profiling`)");
        std::process::exit(2);
    }
}
