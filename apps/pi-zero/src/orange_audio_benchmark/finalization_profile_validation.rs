use crate::dsp_scenarios::ExpectedLiveState;
use realtime_engine::synth::SynthProfileSnapshot;

pub(crate) fn validate_profile_state(
    snapshot: &SynthProfileSnapshot,
    expected: ExpectedLiveState,
    expected_voice_admission_drops: u64,
) -> Result<(), String> {
    let actual = (
        snapshot.active_synth_voices,
        snapshot.active_sample_voices,
        snapshot.active_preview_sample_voices,
        snapshot.active_momentary_fx,
        snapshot.active_bus_fx_slots,
        snapshot.active_global_fx_slots,
        snapshot.cumulative_voice_steals,
    );
    let expected = (
        expected.active_synth_voices,
        expected.active_sample_voices,
        0,
        expected.active_momentary_fx,
        expected.active_bus_fx_slots,
        expected.active_global_fx_slots,
        expected.expected_voice_steals,
    );
    if actual != expected {
        return Err(format!(
            "fixture state mismatch: actual={actual:?} expected={expected:?}"
        ));
    }
    if snapshot.cumulative_voice_admission_drops != expected_voice_admission_drops {
        return Err(format!(
            "fixture state mismatch: voice admission drops actual={} expected={}",
            snapshot.cumulative_voice_admission_drops, expected_voice_admission_drops
        ));
    }
    Ok(())
}
