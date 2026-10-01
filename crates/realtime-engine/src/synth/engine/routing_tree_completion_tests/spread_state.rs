use crate::synth::engine::routing_tree_executor_state_tests::engine_source_state_signature;
use crate::synth::engine::routing_tree_executor_test_support::assert_ulp_close;
use crate::synth::engine::routing_tree_executor_test_support::bus_chain_state;
use crate::synth::engine::{SourceWorkerRuntime, SynthEngine};

pub(super) type BusOutputSpreadStateSignature = Vec<(usize, usize, Vec<f32>)>;

pub(super) fn recovered_source_state_signature(
    runtime: &mut SourceWorkerRuntime,
    engine: &mut SynthEngine,
) -> (String, Vec<String>, BusOutputSpreadStateSignature) {
    runtime
        .with_recovered_routing_tree_owners(engine, |engine| {
            (
                engine_source_state_signature(engine),
                bus_chain_state(engine),
                bus_output_spread_state_signature(engine),
            )
        })
        .expect("recovered source owners")
}

pub(super) fn bus_output_spread_state_signature(
    engine: &SynthEngine,
) -> BusOutputSpreadStateSignature {
    engine
        .bus_output_spread_state
        .iter()
        .enumerate()
        .map(|(bus, state)| {
            let (idx, buf) = state.state_for_test();
            (bus, idx, buf.to_vec())
        })
        .collect()
}

pub(super) fn assert_spread_state_matches(
    actual: &BusOutputSpreadStateSignature,
    expected: &BusOutputSpreadStateSignature,
    max_ulps: u32,
    label: &str,
) {
    assert_eq!(actual.len(), expected.len(), "{label} bus count");
    for ((actual_bus, actual_idx, actual_buf), (expected_bus, expected_idx, expected_buf)) in
        actual.iter().zip(expected)
    {
        assert_eq!(actual_bus, expected_bus, "{label} bus");
        assert_eq!(
            actual_buf.len(),
            expected_buf.len(),
            "{label} buffer length"
        );
        for frame in 0..actual_buf.len() {
            let actual = actual_buf[(actual_idx + frame) % actual_buf.len()];
            let expected = expected_buf[(expected_idx + frame) % expected_buf.len()];
            assert_ulp_close(
                actual,
                expected,
                max_ulps,
                &format!("{label} bus {actual_bus} frame {frame}"),
            );
        }
    }
}

pub(super) fn assert_spread_state_is_nontrivial(
    state: &BusOutputSpreadStateSignature,
    label: &str,
) {
    assert!(
        state
            .iter()
            .any(|(_, _, buffer)| buffer.iter().any(|sample| *sample != 0.0)),
        "{label} spread state remained zero"
    );
}
