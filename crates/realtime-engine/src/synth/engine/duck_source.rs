use super::super::fx_params::{DuckSource, DuckSourceTap, FxBusParams};
use super::super::types::INSTRUMENT_SLOT_COUNT;

pub(super) fn resolve_duck_source(
    params: FxBusParams,
    instrument_out: &[f32; INSTRUMENT_SLOT_COUNT],
    instrument_volume: &[f32; INSTRUMENT_SLOT_COUNT],
    bus_input: &[f32],
    bus_volume: &[f32],
) -> f32 {
    let FxBusParams::Duck {
        source, source_tap, ..
    } = params
    else {
        return 0.0;
    };
    match source {
        DuckSource::Instrument(index) => resolve_duck_source_value(
            source,
            source_tap,
            instrument_out.get(index).copied().unwrap_or(0.0),
            instrument_volume.get(index).copied().unwrap_or(1.0),
            0.0,
            1.0,
        ),
        DuckSource::Bus(index) => resolve_duck_source_value(
            source,
            source_tap,
            0.0,
            1.0,
            bus_input.get(index).copied().unwrap_or(0.0),
            bus_volume.get(index).copied().unwrap_or(1.0),
        ),
    }
}

pub(super) fn resolve_duck_source_value(
    source: DuckSource,
    source_tap: DuckSourceTap,
    instrument_out: f32,
    instrument_volume: f32,
    bus_input: f32,
    bus_volume: f32,
) -> f32 {
    match (source, source_tap) {
        (DuckSource::Instrument(_), DuckSourceTap::Pre) => instrument_out,
        (DuckSource::Instrument(_), DuckSourceTap::Post) => instrument_out * instrument_volume,
        (DuckSource::Bus(_), DuckSourceTap::Pre) => bus_input,
        (DuckSource::Bus(_), DuckSourceTap::Post) => bus_input * bus_volume,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::fx_params::{DuckSource, DuckSourceTap};

    #[test]
    fn pre_taps_return_raw_values_and_post_taps_apply_only_the_source_fader() {
        let instrument = 0.75_f32;
        let bus = -0.5_f32;
        assert_eq!(
            resolve_duck_source_value(
                DuckSource::Instrument(0),
                DuckSourceTap::Pre,
                instrument,
                0.25,
                bus,
                0.7,
            )
            .to_bits(),
            instrument.to_bits()
        );
        assert_eq!(
            resolve_duck_source_value(
                DuckSource::Instrument(0),
                DuckSourceTap::Post,
                instrument,
                0.25,
                bus,
                0.7,
            )
            .to_bits(),
            (instrument * 0.25).to_bits()
        );
        assert_eq!(
            resolve_duck_source_value(
                DuckSource::Bus(0),
                DuckSourceTap::Pre,
                instrument,
                0.25,
                bus,
                0.7,
            )
            .to_bits(),
            bus.to_bits()
        );
        assert_eq!(
            resolve_duck_source_value(
                DuckSource::Bus(0),
                DuckSourceTap::Post,
                instrument,
                0.25,
                bus,
                0.7,
            )
            .to_bits(),
            (bus * 0.7).to_bits()
        );
    }
}
