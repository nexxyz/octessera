use super::*;

pub(in crate::synth) struct FilterLfoParams {
    pub(in crate::synth) kind: FilterLfoKind,
    pub(in crate::synth) rate_hz: f32,
    pub(in crate::synth) depth: f32,
    pub(in crate::synth) center_hz: f32,
    pub(in crate::synth) q: f32,
}

#[derive(Clone, Copy, Debug)]
pub(in crate::synth) struct FilterLfoCache {
    kind: FilterLfoKind,
    rate_hz: f32,
    sample_rate: u32,
    phase_inc: f32,
    mode: FilterType,
}

impl FilterLfoCache {
    pub(in crate::synth) fn new(kind: FilterLfoKind, rate_hz: f32, sample_rate: u32) -> Self {
        Self {
            kind,
            rate_hz,
            sample_rate,
            phase_inc: 2.0 * PI * rate_hz / sample_rate as f32,
            mode: filter_lfo_mode(kind),
        }
    }

    fn refresh(&mut self, kind: FilterLfoKind, rate_hz: f32, sample_rate: u32) {
        if std::mem::discriminant(&self.kind) == std::mem::discriminant(&kind)
            && self.rate_hz.to_bits() == rate_hz.to_bits()
            && self.sample_rate == sample_rate
        {
            return;
        }
        *self = Self::new(kind, rate_hz, sample_rate);
    }
}

pub(in crate::synth) fn process_filter_lfo(
    state: &mut FxBusState,
    input: f32,
    params: FilterLfoParams,
    sample_rate: u32,
) -> f32 {
    let FxBusState::FilterLfo { filt, phase, cache } = state else {
        *state = FxBusState::FilterLfo {
            filt: BiquadState::new(),
            phase: 0.0,
            cache: FilterLfoCache::new(params.kind, params.rate_hz, sample_rate),
        };
        return input;
    };
    cache.refresh(params.kind, params.rate_hz, sample_rate);
    process_filter_lfo_channel(
        filt,
        phase,
        cache.phase_inc,
        cache.mode,
        input,
        &params,
        sample_rate,
    )
}

fn process_filter_lfo_channel(
    filt: &mut BiquadState,
    phase: &mut f32,
    phase_inc: f32,
    mode: FilterType,
    input: f32,
    params: &FilterLfoParams,
    sample_rate: u32,
) -> f32 {
    let sweep = ((*phase).sin() + 1.0) * 0.5;
    let semis = (sweep - 0.5) * 48.0 * params.depth;
    let cutoff = (params.center_hz * 2.0_f32.powf(semis / 12.0)).clamp(40.0, 18_000.0);
    *phase = wrap_phase(*phase + phase_inc);
    filt.process(input, mode, cutoff, params.q, sample_rate)
        .clamp(-1.5, 1.5)
}

fn filter_lfo_mode(kind: FilterLfoKind) -> FilterType {
    match kind {
        FilterLfoKind::Wah => FilterType::Bandpass,
        FilterLfoKind::FilterLfo => FilterType::Lowpass,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracted_filter_lfo_matches_reference_with_param_changes() {
        let params = [
            FilterLfoParams {
                kind: FilterLfoKind::FilterLfo,
                rate_hz: 0.4,
                depth: 0.55,
                center_hz: 1600.0,
                q: 1.0,
            },
            FilterLfoParams {
                kind: FilterLfoKind::FilterLfo,
                rate_hz: 1.2,
                depth: 0.25,
                center_hz: 900.0,
                q: 0.8,
            },
        ];
        assert_matches_reference(&params, 4096);
    }

    #[test]
    fn extracted_wah_matches_reference_with_param_changes() {
        let params = [
            FilterLfoParams {
                kind: FilterLfoKind::Wah,
                rate_hz: 0.9,
                depth: 0.75,
                center_hz: 1200.0,
                q: 1.4,
            },
            FilterLfoParams {
                kind: FilterLfoKind::Wah,
                rate_hz: 2.1,
                depth: 0.35,
                center_hz: 2000.0,
                q: 0.7,
            },
        ];
        assert_matches_reference(&params, 4096);
    }

    fn assert_matches_reference(params: &[FilterLfoParams], frames: usize) {
        let mut actual_filter = BiquadState::new();
        let mut expected_filter = BiquadState::new();
        let mut actual_phase = 0.0;
        let mut expected_phase = 0.0;
        let mut cache = FilterLfoCache::new(params[0].kind, params[0].rate_hz, 44_100);
        for frame in 0..frames {
            let input =
                ((frame as f32) * 0.019).sin() * 0.4 + ((frame as f32) * 0.037).cos() * 0.15;
            let params = &params[(frame * params.len()) / frames];
            cache.refresh(params.kind, params.rate_hz, 44_100);
            let actual = process_filter_lfo_channel(
                &mut actual_filter,
                &mut actual_phase,
                cache.phase_inc,
                cache.mode,
                input,
                params,
                44_100,
            );
            let expected = reference_process_filter_lfo_channel(
                &mut expected_filter,
                &mut expected_phase,
                input,
                params,
                44_100,
            );
            assert_eq!(actual.to_bits(), expected.to_bits(), "sample {frame}");
            assert_eq!(
                actual_phase.to_bits(),
                expected_phase.to_bits(),
                "phase {frame}"
            );
        }
    }

    #[test]
    fn cache_refresh_matches_reference_when_kind_rate_and_sample_rate_change() {
        let params = [
            (
                FilterLfoParams {
                    kind: FilterLfoKind::FilterLfo,
                    rate_hz: 0.4,
                    depth: 0.55,
                    center_hz: 1600.0,
                    q: 1.0,
                },
                44_100,
            ),
            (
                FilterLfoParams {
                    kind: FilterLfoKind::Wah,
                    rate_hz: 1.1,
                    depth: 0.35,
                    center_hz: 1200.0,
                    q: 0.8,
                },
                48_000,
            ),
        ];
        let mut actual_filter = BiquadState::new();
        let mut expected_filter = BiquadState::new();
        let mut actual_phase = 0.0;
        let mut expected_phase = 0.0;
        let mut cache = FilterLfoCache::new(params[0].0.kind, params[0].0.rate_hz, params[0].1);

        for frame in 0..4096 {
            let (params, sample_rate) = &params[(frame * params.len()) / 4096];
            cache.refresh(params.kind, params.rate_hz, *sample_rate);
            let input = ((frame as f32) * 0.013).sin() * 0.3 + ((frame as f32) * 0.029).cos() * 0.2;
            let actual = process_filter_lfo_channel(
                &mut actual_filter,
                &mut actual_phase,
                cache.phase_inc,
                cache.mode,
                input,
                params,
                *sample_rate,
            );
            let expected = reference_process_filter_lfo_channel(
                &mut expected_filter,
                &mut expected_phase,
                input,
                params,
                *sample_rate,
            );
            assert_eq!(actual.to_bits(), expected.to_bits(), "sample {frame}");
            assert_eq!(
                actual_phase.to_bits(),
                expected_phase.to_bits(),
                "phase {frame}"
            );
        }
    }

    fn reference_process_filter_lfo_channel(
        filt: &mut BiquadState,
        phase: &mut f32,
        input: f32,
        params: &FilterLfoParams,
        sample_rate: u32,
    ) -> f32 {
        let sweep = ((*phase).sin() + 1.0) * 0.5;
        let semis = (sweep - 0.5) * 48.0 * params.depth;
        let cutoff = (params.center_hz * 2.0_f32.powf(semis / 12.0)).clamp(40.0, 18_000.0);
        *phase = wrap_phase(*phase + 2.0 * PI * params.rate_hz / sample_rate as f32);
        let mode = match params.kind {
            FilterLfoKind::Wah => FilterType::Bandpass,
            FilterLfoKind::FilterLfo => FilterType::Lowpass,
        };
        filt.process(input, mode, cutoff, params.q, sample_rate)
            .clamp(-1.5, 1.5)
    }
}
