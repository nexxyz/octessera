use super::SynthEngine;
use crate::synth::types::{BUS_COUNT, INSTRUMENT_SLOT_COUNT};

const MIXER_GAIN_RAMP_MS: f32 = 5.0;

/// One gain moving linearly from `start` to `target`; settled once `elapsed` reaches the ramp
/// length, when the engine's gain field holds the target.
#[derive(Clone, Copy, Debug)]
struct GainRamp {
    start: f32,
    step: f32,
    target: f32,
    elapsed: u32,
}

impl GainRamp {
    const SETTLED: Self = Self {
        start: 0.0,
        step: 0.0,
        target: 0.0,
        elapsed: u32::MAX,
    };

    fn target_or(&self, current: f32, len: u32) -> f32 {
        if self.elapsed < len {
            self.target
        } else {
            current
        }
    }

    /// Starts from the gain the next frame would use. A settled gain already played that value,
    /// so it moves one step at once; a running ramp plays its pending value first.
    fn start(&mut self, current: &mut f32, target: f32, len: u32) {
        let settled = self.elapsed >= len;
        *self = Self {
            start: *current,
            step: (target - *current) / len as f32,
            target,
            elapsed: 0,
        };
        if settled {
            self.advance(current, 1, len);
        }
    }

    fn value(&self, settled: f32, offset: u32, len: u32) -> f32 {
        if self.elapsed >= len {
            return settled;
        }
        let frame = self.elapsed.saturating_add(offset);
        if frame >= len {
            self.target
        } else {
            self.start + self.step * frame as f32
        }
    }

    fn advance(&mut self, current: &mut f32, frames: u32, len: u32) {
        if self.elapsed >= len {
            return;
        }
        self.elapsed = self.elapsed.saturating_add(frames);
        *current = self.value(self.target, 0, len);
    }

    fn finish(&mut self, current: &mut f32, len: u32) {
        if self.elapsed < len {
            *current = self.target;
        }
        *self = Self::SETTLED;
    }
}

#[cfg(feature = "routing-tree-executor")]
fn pair_value(ramps: &[GainRamp; 2], settled: (f32, f32), offset: u32, len: u32) -> (f32, f32) {
    (
        ramps[0].value(settled.0, offset, len),
        ramps[1].value(settled.1, offset, len),
    )
}

fn start_pair(ramps: &mut [GainRamp; 2], current: &mut (f32, f32), target: (f32, f32), len: u32) {
    ramps[0].start(&mut current.0, target.0, len);
    ramps[1].start(&mut current.1, target.1, len);
}

fn advance_pair(ramps: &mut [GainRamp; 2], current: &mut (f32, f32), frames: u32, len: u32) {
    ramps[0].advance(&mut current.0, frames, len);
    ramps[1].advance(&mut current.1, frames, len);
}

fn finish_pair(ramps: &mut [GainRamp; 2], current: &mut (f32, f32), len: u32) {
    ramps[0].finish(&mut current.0, len);
    ramps[1].finish(&mut current.1, len);
}

/// Linear ramps for the master, instrument and bus volume and pan gains set by live controls.
/// The master ramps on the engine's output frames; the instrument and bus gains ramp on the
/// frames their sources render, which a routing-tree worker reaches through a copy.
#[derive(Clone, Copy, Debug)]
pub(super) struct MixerGainRamps {
    len: u32,
    master_active: bool,
    mix_active: bool,
    master: GainRamp,
    slot_volume: [GainRamp; INSTRUMENT_SLOT_COUNT],
    slot_pan: [[GainRamp; 2]; INSTRUMENT_SLOT_COUNT],
    bus_volume: [GainRamp; BUS_COUNT],
    bus_pan: [[GainRamp; 2]; BUS_COUNT],
}

impl MixerGainRamps {
    pub(super) fn new(sample_rate: u32) -> Self {
        Self {
            len: (sample_rate as f32 * MIXER_GAIN_RAMP_MS / 1000.0)
                .round()
                .max(1.0) as u32,
            master_active: false,
            mix_active: false,
            master: GainRamp::SETTLED,
            slot_volume: [GainRamp::SETTLED; INSTRUMENT_SLOT_COUNT],
            slot_pan: [[GainRamp::SETTLED; 2]; INSTRUMENT_SLOT_COUNT],
            bus_volume: [GainRamp::SETTLED; BUS_COUNT],
            bus_pan: [[GainRamp::SETTLED; 2]; BUS_COUNT],
        }
    }

    #[inline]
    pub(super) fn mix_active(&self) -> bool {
        self.mix_active
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn slot_gains_at(
        &self,
        offset: u32,
        volume: &[f32; INSTRUMENT_SLOT_COUNT],
        pan: &[(f32, f32); INSTRUMENT_SLOT_COUNT],
    ) -> (
        [f32; INSTRUMENT_SLOT_COUNT],
        [(f32, f32); INSTRUMENT_SLOT_COUNT],
    ) {
        (
            std::array::from_fn(|slot| {
                self.slot_volume[slot].value(volume[slot], offset, self.len)
            }),
            std::array::from_fn(|slot| {
                pair_value(&self.slot_pan[slot], pan[slot], offset, self.len)
            }),
        )
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn bus_gains_at(
        &self,
        bus: usize,
        offset: u32,
        volume: f32,
        pan: (f32, f32),
    ) -> (f32, (f32, f32)) {
        (
            self.bus_volume[bus].value(volume, offset, self.len),
            pair_value(&self.bus_pan[bus], pan, offset, self.len),
        )
    }

    fn refresh_mix_active(&mut self) {
        let len = self.len;
        let active = |ramp: &GainRamp| ramp.elapsed < len;
        self.mix_active = self.slot_volume.iter().any(active)
            || self.slot_pan.iter().flatten().any(active)
            || self.bus_volume.iter().any(active)
            || self.bus_pan.iter().flatten().any(active);
    }
}

impl SynthEngine {
    pub(super) fn master_volume_target(&self) -> f32 {
        let ramps = &self.mixer_gain_ramps;
        ramps.master.target_or(self.master_volume, ramps.len)
    }

    pub(super) fn slot_volume_target(&self, slot: usize) -> f32 {
        let ramps = &self.mixer_gain_ramps;
        ramps.slot_volume[slot].target_or(self.slot_volume[slot], ramps.len)
    }

    pub(super) fn bus_volume_target(&self, bus: usize) -> f32 {
        let ramps = &self.mixer_gain_ramps;
        match ramps.bus_volume.get(bus) {
            Some(ramp) => ramp.target_or(self.bus_mix.volume[bus], ramps.len),
            None => self.bus_mix.volume[bus],
        }
    }

    pub(super) fn ramp_master_volume(&mut self, target: f32) {
        let ramps = &mut self.mixer_gain_ramps;
        ramps
            .master
            .start(&mut self.master_volume, target, ramps.len);
        ramps.master_active = ramps.master.elapsed < ramps.len;
    }

    pub(super) fn ramp_slot_volume(&mut self, slot: usize, target: f32) {
        let ramps = &mut self.mixer_gain_ramps;
        ramps.slot_volume[slot].start(&mut self.slot_volume[slot], target, ramps.len);
        ramps.refresh_mix_active();
    }

    pub(super) fn ramp_slot_pan(&mut self, slot: usize, target: (f32, f32)) {
        let ramps = &mut self.mixer_gain_ramps;
        start_pair(
            &mut ramps.slot_pan[slot],
            &mut self.slot_pan_gains[slot],
            target,
            ramps.len,
        );
        ramps.refresh_mix_active();
    }

    pub(super) fn ramp_bus_volume(&mut self, bus: usize, target: f32) {
        let ramps = &mut self.mixer_gain_ramps;
        match ramps.bus_volume.get_mut(bus) {
            Some(ramp) => ramp.start(&mut self.bus_mix.volume[bus], target, ramps.len),
            None => self.bus_mix.volume[bus] = target,
        }
        ramps.refresh_mix_active();
    }

    pub(super) fn ramp_bus_pan(&mut self, bus: usize, target: (f32, f32)) {
        let ramps = &mut self.mixer_gain_ramps;
        match ramps.bus_pan.get_mut(bus) {
            Some(ramp) => start_pair(ramp, &mut self.bus_mix.pan_gains[bus], target, ramps.len),
            None => self.bus_mix.pan_gains[bus] = target,
        }
        ramps.refresh_mix_active();
    }

    #[inline]
    pub(super) fn advance_master_gain_ramp(&mut self) {
        if !self.mixer_gain_ramps.master_active {
            return;
        }
        let ramps = &mut self.mixer_gain_ramps;
        ramps.master.advance(&mut self.master_volume, 1, ramps.len);
        ramps.master_active = ramps.master.elapsed < ramps.len;
    }

    pub(super) fn advance_mix_gain_ramps(&mut self, frames: u32) {
        let ramps = &mut self.mixer_gain_ramps;
        let len = ramps.len;
        for (ramp, volume) in ramps.slot_volume.iter_mut().zip(&mut self.slot_volume) {
            ramp.advance(volume, frames, len);
        }
        for (ramp, pan) in ramps.slot_pan.iter_mut().zip(&mut self.slot_pan_gains) {
            advance_pair(ramp, pan, frames, len);
        }
        for (ramp, volume) in ramps.bus_volume.iter_mut().zip(&mut self.bus_mix.volume) {
            ramp.advance(volume, frames, len);
        }
        for (ramp, pan) in ramps.bus_pan.iter_mut().zip(&mut self.bus_mix.pan_gains) {
            advance_pair(ramp, pan, frames, len);
        }
        ramps.refresh_mix_active();
    }

    /// Moves the instrument and bus ramps past a block the routing-tree workers now render.
    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn advance_dispatched_mix_gain_ramps(&mut self, frames: usize) {
        if self.mixer_gain_ramps.mix_active {
            self.advance_mix_gain_ramps(frames as u32);
        }
    }

    /// Lands every running ramp on its target before a config or patch load writes its gains.
    pub(super) fn finish_mixer_gain_ramps(&mut self) {
        let ramps = &mut self.mixer_gain_ramps;
        let len = ramps.len;
        ramps.master.finish(&mut self.master_volume, len);
        for (ramp, volume) in ramps.slot_volume.iter_mut().zip(&mut self.slot_volume) {
            ramp.finish(volume, len);
        }
        for (ramp, pan) in ramps.slot_pan.iter_mut().zip(&mut self.slot_pan_gains) {
            finish_pair(ramp, pan, len);
        }
        for (ramp, volume) in ramps.bus_volume.iter_mut().zip(&mut self.bus_mix.volume) {
            ramp.finish(volume, len);
        }
        for (ramp, pan) in ramps.bus_pan.iter_mut().zip(&mut self.bus_mix.pan_gains) {
            finish_pair(ramp, pan, len);
        }
        ramps.master_active = false;
        ramps.mix_active = false;
    }
}

#[cfg(test)]
mod tests;
