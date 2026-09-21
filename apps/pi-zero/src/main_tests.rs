use super::raspberry_benchmark_requested_from;

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
use super::orange_benchmark_requested_from;

#[test]
fn raspberry_benchmark_argument_is_detected_for_profile_rejection() {
    assert!(raspberry_benchmark_requested_from(
        ["--benchmark-raspberry-audio".into()].into_iter()
    ));
    assert!(!raspberry_benchmark_requested_from(
        ["--print-build-metadata".into()].into_iter()
    ));
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
#[test]
fn orange_benchmark_argument_is_detected_for_profile_rejection() {
    assert!(orange_benchmark_requested_from(
        ["--benchmark-orange-audio".into()].into_iter()
    ));
    assert!(!orange_benchmark_requested_from(
        ["--benchmark-raspberry-audio".into()].into_iter()
    ));
}
