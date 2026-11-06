use super::{cell_for_key, decode_momentary, encode_momentary, key_for_cell, LatestKey};
use realtime_engine::synth::{
    DrumParamId, PluckParamId, PreparedMomentaryFxUpdate, SynthParamId, INSTRUMENT_SLOT_COUNT,
};

#[test]
fn synth_oscillator_scalar_cells_round_trip_without_colliding_with_other_owners() {
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for param in SynthParamId::ALL {
            let key = LatestKey::SynthParam(slot, param);
            assert_eq!(key_for_cell(cell_for_key(key)), key);
        }
    }
    assert_ne!(
        cell_for_key(LatestKey::SynthParam(0, SynthParamId::Osc1LevelPct)),
        cell_for_key(LatestKey::SynthParam(1, SynthParamId::Osc1LevelPct))
    );
    assert_ne!(
        cell_for_key(LatestKey::SynthParam(0, SynthParamId::Osc2PulseWidthPct)),
        cell_for_key(LatestKey::FmParam(
            0,
            realtime_engine::synth::FmParamId::Index
        ))
    );
}

#[test]
fn pluck_scalar_cells_round_trip_and_keep_independent_generations() {
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for param in PluckParamId::ALL {
            let key = LatestKey::PluckParam(slot, param);
            assert_eq!(key_for_cell(cell_for_key(key)), key);
        }
    }
    assert_ne!(
        cell_for_key(LatestKey::PluckParam(0, PluckParamId::DecayMs)),
        cell_for_key(LatestKey::PluckParam(1, PluckParamId::DecayMs))
    );
    assert_ne!(
        cell_for_key(LatestKey::PluckParam(0, PluckParamId::BrightnessPct)),
        cell_for_key(LatestKey::FmParam(
            0,
            realtime_engine::synth::FmParamId::Index
        ))
    );
}

#[test]
fn drum_cells_are_unique_by_slot_voice_and_param() {
    let mut cells = std::collections::BTreeSet::new();
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for voice in 0..8 {
            for param in DrumParamId::ALL {
                let key = LatestKey::DrumParam(slot, voice, param);
                let cell = cell_for_key(key);
                assert_eq!(key_for_cell(cell), key);
                assert!(cells.insert(cell));
            }
        }
    }
    assert_ne!(
        cell_for_key(LatestKey::DrumParam(0, 0, DrumParamId::DecayMs)),
        cell_for_key(LatestKey::FmParam(
            0,
            realtime_engine::synth::FmParamId::Index
        ))
    );
}

#[test]
fn pitch_update_encoding_round_trips_finite_concrete_values() {
    let update = PreparedMomentaryFxUpdate::PitchShift {
        epoch: 7,
        target_octaves: (-5.0 + 0.25) / 12.0,
        mix: 0.65,
        slide_in_len: 5_292,
        slide_out_len: 7_938,
    };
    let (kind, values) = encode_momentary(update);
    let PreparedMomentaryFxUpdate::PitchShift {
        epoch,
        target_octaves,
        mix,
        slide_in_len,
        slide_out_len,
    } = decode_momentary(7, kind, values).expect("pitch update")
    else {
        panic!("decoded non-pitch update");
    };
    assert_eq!(epoch, 7);
    assert!(target_octaves.is_finite());
    assert!(mix.is_finite());
    assert_eq!(
        target_octaves.to_bits(),
        ((-5.0_f32 + 0.25) / 12.0_f32).to_bits()
    );
    assert_eq!(mix.to_bits(), 0.65_f32.to_bits());
    assert_eq!(slide_in_len, 5_292);
    assert_eq!(slide_out_len, 7_938);
}

#[test]
fn every_momentary_type_update_round_trips_through_its_cell() {
    use realtime_engine::synth::prepare_momentary_fx_update;
    use serde_json::json;
    let params = |pairs: &[(&str, f64)]| {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), json!(value)))
            .collect::<std::collections::BTreeMap<_, _>>()
    };
    let updates = [
        (
            "tape_stop",
            params(&[("stopMs", 900.0), ("startMs", 120.0)]),
        ),
        ("reverse", params(&[("lengthMs", 320.0), ("mixPct", 65.0)])),
        (
            "trance_gate",
            params(&[("ratePulses", 4.0), ("depthPct", 80.0), ("tempoBpm", 133.0)]),
        ),
        (
            "dub_throw",
            params(&[
                ("timePulses", 18.0),
                ("feedbackPct", 70.0),
                ("tempoBpm", 97.0),
            ]),
        ),
        (
            "wash",
            params(&[("sizePct", 35.0), ("mixPct", 45.0), ("releaseMs", 4_000.0)]),
        ),
        (
            "hp_riser",
            params(&[
                ("cutoffPct", 75.0),
                ("resonancePct", 55.0),
                ("sweepOutMs", 400.0),
            ]),
        ),
        (
            "crush",
            params(&[("bits", 3.0), ("rateDiv", 12.0), ("mixPct", 50.0)]),
        ),
    ];
    let mut kinds = Vec::new();
    for (fx_type, params) in updates {
        let update = prepare_momentary_fx_update(9, fx_type.into(), params, 48_000).expect(fx_type);
        let (kind, values) = encode_momentary(update);
        assert_eq!(decode_momentary(9, kind, values), Some(update), "{fx_type}");
        kinds.push(kind);
    }
    kinds.sort_unstable();
    kinds.dedup();
    assert_eq!(kinds.len(), 7, "each type has its own cell kind");
}
