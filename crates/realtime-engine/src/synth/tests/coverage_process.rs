use super::super::fx::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, process_fx_bus_slot, process_master_fx_slot, FxBusState,
    MasterFxState,
};
use super::super::fx_params::{DuckSource, FilterLfoKind, FxBusParams};
use super::*;

#[test]
fn fx_state_factories_and_matchers_cover_supported_variants() {
    let params = [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 2.0,
            depth: 0.5,
        },
        FxBusParams::Delay {
            time_ms: 20.0,
            feedback: 0.2,
            mix: 0.3,
            spread: 0.0,
        },
        FxBusParams::ModDelay {
            rate_hz: 0.5,
            depth_ms: 3.0,
            base_ms: 8.0,
            feedback: 0.0,
            mix: 0.2,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::FilterLfo,
            rate_hz: 0.5,
            depth: 0.7,
            center_hz: 1_000.0,
            q: 0.8,
        },
        FxBusParams::Reverb {
            mix: 0.4,
            decay: 0.8,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 10.0,
            mix: 0.6,
        },
        FxBusParams::AutoPan {
            rate_hz: 0.5,
            depth: 1.0,
        },
        FxBusParams::Duck {
            source: DuckSource::Instrument(0),
            threshold: 0.1,
            amount: 0.5,
            attack_ms: 10.0,
            release_ms: 50.0,
        },
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in params {
        let state = fx_bus_state_from_params(&param, 48_000);
        assert!(fx_bus_state_matches_params(&state, &param));
    }

    let master_params = [
        FxBusParams::None,
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in master_params {
        let state = master_fx_state_from_params(&param);
        assert!(master_fx_state_matches_params(&state, &param));
    }

    assert!(!fx_bus_state_matches_params(
        &FxBusState::Tremolo { phase: 0.0 },
        &FxBusParams::Delay {
            time_ms: 10.0,
            feedback: 0.0,
            mix: 0.0,
            spread: 0.0,
        },
    ));
    assert!(!master_fx_state_matches_params(
        &MasterFxState::Compressor { env: 0.0 },
        &FxBusParams::Eq {
            low_gain_db: 0.0,
            mid_gain_db: 0.0,
            mid_freq_hz: 1_000.0,
            mid_q: 1.0,
            high_gain_db: 0.0,
            mix: 1.0,
        },
    ));
}

#[test]
fn process_fx_paths_stay_finite_across_bus_and_master_slots() {
    let slot_out = [0.75; INSTRUMENT_SLOT_COUNT];
    let bus_in = [0.5, 0.25];
    let bus_params = [
        FxBusParams::None,
        FxBusParams::Tremolo {
            rate_hz: 2.0,
            depth: 0.8,
        },
        FxBusParams::Delay {
            time_ms: 5.0,
            feedback: 0.3,
            mix: 0.5,
            spread: 0.0,
        },
        FxBusParams::ModDelay {
            rate_hz: 0.7,
            depth_ms: 4.0,
            base_ms: 8.0,
            feedback: 0.2,
            mix: 0.5,
        },
        FxBusParams::FilterLfo {
            kind: FilterLfoKind::Wah,
            rate_hz: 1.2,
            depth: 0.7,
            center_hz: 900.0,
            q: 6.0,
        },
        FxBusParams::Reverb {
            mix: 0.4,
            decay: 0.7,
            damp: 0.2,
        },
        FxBusParams::Glitch {
            chance: 1.0,
            slice_ms: 8.0,
            mix: 1.0,
        },
        FxBusParams::AutoPan {
            rate_hz: 0.5,
            depth: 1.0,
        },
        FxBusParams::Duck {
            source: DuckSource::Bus(0),
            threshold: 0.1,
            amount: 0.6,
            attack_ms: 5.0,
            release_ms: 20.0,
        },
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Bitcrusher {
            rate_div: 4,
            bits: 6,
            mix: 0.5,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in bus_params {
        let mut state = FxBusState::None;
        let first = process_fx_bus_slot(&param, &mut state, 0.5, &slot_out, &bus_in, 48_000);
        let second = process_fx_bus_slot(&param, &mut state, 0.25, &slot_out, &bus_in, 48_000);
        assert!(first.is_finite());
        assert!(second.is_finite());
    }

    let master_params = [
        FxBusParams::None,
        FxBusParams::Saturator {
            drive: 2.0,
            mix: 1.0,
        },
        FxBusParams::Distortion {
            drive: 3.0,
            clip: 0.4,
            mix: 0.7,
        },
        FxBusParams::Compressor {
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 80.0,
            makeup_db: 2.0,
            mix: 0.9,
        },
        FxBusParams::Eq {
            low_gain_db: 3.0,
            mid_gain_db: -2.0,
            mid_freq_hz: 1_200.0,
            mid_q: 1.1,
            high_gain_db: 4.0,
            mix: 1.0,
        },
        FxBusParams::Vinyl {
            saturation: 0.2,
            crackle: 0.1,
            warp_depth: 0.3,
            mix: 0.8,
        },
    ];

    for param in master_params {
        let mut state = MasterFxState::None;
        let first = process_master_fx_slot(&param, &mut state, 0.5, -0.25, 48_000);
        let second = process_master_fx_slot(&param, &mut state, 0.25, -0.5, 48_000);
        assert!(first.0.is_finite() && first.1.is_finite());
        assert!(second.0.is_finite() && second.1.is_finite());
    }
}
