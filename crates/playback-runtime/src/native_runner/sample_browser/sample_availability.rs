use crate::native_menu::NativeSampleAvailability;

use super::super::{same_sample_path, NativeRunner};

impl NativeRunner {
    pub(in crate::native_runner) fn mark_sample_available(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
    ) {
        let Some(availability) = self
            .sample_availability
            .get_mut(instrument_slot)
            .and_then(|slots| slots.get_mut(sample_slot))
        else {
            return;
        };
        *availability = NativeSampleAvailability::Available;
    }

    pub(in crate::native_runner) fn mark_sample_unavailable(
        &mut self,
        instrument_slot: usize,
        sample_slot: usize,
    ) {
        if let Some(availability) = self
            .sample_availability
            .get_mut(instrument_slot)
            .and_then(|slots| slots.get_mut(sample_slot))
        {
            *availability = NativeSampleAvailability::Unavailable;
        }
    }

    pub(in crate::native_runner) fn mark_sample_unavailable_from_error(
        &mut self,
        error: &crate::protocol::RuntimeErrorFacts,
    ) -> Vec<usize> {
        if error.domain != crate::RuntimeErrorDomain::Sample {
            return Vec::new();
        }
        let Some(failed_path) = error
            .message
            .as_deref()
            .and_then(crate::SampleLoadError::path_in_message)
        else {
            return Vec::new();
        };
        let mut changed = Vec::new();
        let matching_slots =
            self.instruments
                .iter()
                .enumerate()
                .flat_map(|(instrument_slot, instrument)| {
                    instrument.sample_paths.iter().enumerate().filter_map(
                        move |(sample_slot, path)| {
                            path.as_deref()
                                .filter(|path| same_sample_path(path, failed_path))
                                .map(|_| (instrument_slot, sample_slot))
                        },
                    )
                })
                .collect::<Vec<_>>();
        for (instrument_slot, sample_slot) in matching_slots {
            let unavailable = self
                .sample_availability
                .get(instrument_slot)
                .and_then(|slots| slots.get(sample_slot))
                == Some(&NativeSampleAvailability::Unavailable);
            if !unavailable {
                self.mark_sample_unavailable(instrument_slot, sample_slot);
                if !changed.contains(&instrument_slot) {
                    changed.push(instrument_slot);
                }
            }
        }
        changed
    }

    pub(in crate::native_runner) fn preserve_sample_availability_from(
        &mut self,
        source: &NativeRunner,
    ) {
        for (instrument_slot, instrument) in self.instruments.iter().enumerate() {
            for (sample_slot, path) in instrument.sample_paths.iter().enumerate() {
                let same_path = path
                    .as_deref()
                    .zip(
                        source
                            .instruments
                            .get(instrument_slot)
                            .and_then(|instrument| instrument.sample_paths.get(sample_slot))
                            .and_then(Option::as_deref),
                    )
                    .is_some_and(|(left, right)| same_sample_path(left, right));
                if !same_path {
                    if let Some(availability) = self
                        .sample_availability
                        .get_mut(instrument_slot)
                        .and_then(|slots| slots.get_mut(sample_slot))
                    {
                        *availability = NativeSampleAvailability::Unknown;
                    }
                    continue;
                }
                if let Some(availability) = self
                    .sample_availability
                    .get_mut(instrument_slot)
                    .and_then(|slots| slots.get_mut(sample_slot))
                {
                    *availability = source
                        .sample_availability
                        .get(instrument_slot)
                        .and_then(|slots| slots.get(sample_slot))
                        .copied()
                        .unwrap_or(NativeSampleAvailability::Unknown);
                }
            }
        }
    }
}
