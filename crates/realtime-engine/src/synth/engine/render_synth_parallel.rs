use super::render_voice::{
    refresh_synth_voice_render_cache, render_synth_voice_sample_precomputed,
};
use super::*;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

const MAX_SYNTH_SLOT_WORKERS: usize = 3;
const MIN_PARALLEL_SYNTH_FRAMES: usize = 256;
const MIN_PARALLEL_SYNTH_SLOTS: usize = 3;
const MIN_PARALLEL_SYNTH_VOICES: usize = 3;

pub(super) struct SynthSlotWorkerPool {
    workers: Vec<SynthSlotWorker>,
}

struct SynthSlotWorker {
    shared: Arc<(Mutex<WorkerState>, Condvar)>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
struct SynthSlotJob {
    slot: usize,
    frames: usize,
    base_sample_clock: u64,
    sample_rate: u32,
    active: bool,
    voices: [Voice; VOICES_PER_SLOT],
    config: SynthConfig,
    render_config: SynthVoiceRenderConfig,
    revision: u32,
    mods: InstrumentMod,
}

struct WorkerState {
    jobs: [Option<SynthSlotJob>; INSTRUMENT_SLOT_COUNT],
    samples: [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    active: [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    voices: [Option<[Voice; VOICES_PER_SLOT]>; INSTRUMENT_SLOT_COUNT],
    final_active: [bool; INSTRUMENT_SLOT_COUNT],
    running: bool,
    complete: bool,
    shutdown: bool,
}

impl SynthSlotWorkerPool {
    pub(super) fn start(worker_count: usize) -> Option<Self> {
        let worker_count = worker_count.clamp(1, MAX_SYNTH_SLOT_WORKERS);
        let mut workers = Vec::with_capacity(worker_count);
        for worker_idx in 0..worker_count {
            let shared = Arc::new((Mutex::new(WorkerState::new()), Condvar::new()));
            let thread_shared = Arc::clone(&shared);
            let handle = thread::Builder::new()
                .name(format!("octessera-synth-slot-{worker_idx}"))
                .spawn(move || worker_loop(thread_shared))
                .ok();
            let Some(handle) = handle else {
                stop_workers(&mut workers);
                return None;
            };
            workers.push(SynthSlotWorker {
                shared,
                handle: Some(handle),
            });
        }
        Some(Self { workers })
    }

    pub(super) fn render_synth_slots(
        &mut self,
        frames: usize,
        base_sample_clock: u64,
        sample_rate: u32,
        inputs: &[SynthSlotRenderInput; INSTRUMENT_SLOT_COUNT],
        scratch: &mut BlockSlotScratch,
    ) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES || self.workers.is_empty() {
            return false;
        }
        let Some(schedule) = SynthSlotSchedule::from_inputs(frames, inputs) else {
            return false;
        };
        for worker in &self.workers {
            let (lock, _) = &*worker.shared;
            let Ok(state) = lock.lock() else {
                return false;
            };
            if state.running || state.shutdown {
                return false;
            }
        }
        let mut dispatched = [false; MAX_SYNTH_SLOT_WORKERS];
        for (worker_idx, worker) in self.workers.iter().enumerate() {
            let (lock, condvar) = &*worker.shared;
            let Ok(mut state) = lock.lock() else {
                return false;
            };
            if state.running || state.shutdown {
                return false;
            }
            state.reset();
            let mut assigned = false;
            for index in 0..schedule.len {
                if index % self.workers.len() == worker_idx {
                    let slot = schedule.slots[index];
                    state.jobs[slot] = Some(SynthSlotJob::from_input(
                        slot,
                        frames,
                        base_sample_clock,
                        sample_rate,
                        inputs[slot],
                    ));
                    assigned = true;
                }
            }
            if !assigned {
                continue;
            }
            state.running = true;
            dispatched[worker_idx] = true;
            condvar.notify_one();
        }
        for (worker_idx, worker) in self.workers.iter().enumerate() {
            if !dispatched[worker_idx] {
                continue;
            }
            let (lock, condvar) = &*worker.shared;
            let Ok(mut state) = lock.lock() else {
                return false;
            };
            while !state.complete && !state.shutdown {
                let Ok(next_state) = condvar.wait(state) else {
                    return false;
                };
                state = next_state;
            }
            if state.shutdown {
                return false;
            }
            for slot in 0..INSTRUMENT_SLOT_COUNT {
                if let Some(voices) = state.voices[slot] {
                    scratch.synth_slot_out[slot][..frames]
                        .copy_from_slice(&state.samples[slot][..frames]);
                    scratch.synth_active[slot][..frames]
                        .copy_from_slice(&state.active[slot][..frames]);
                    scratch.synth_voices[slot] = Some(voices);
                    scratch.synth_final_active[slot] = state.final_active[slot];
                }
            }
        }
        true
    }

    pub(super) fn should_render_parallel(
        &self,
        frames: usize,
        inputs: &[SynthSlotRenderInput; INSTRUMENT_SLOT_COUNT],
    ) -> bool {
        SynthSlotSchedule::from_inputs(frames, inputs).is_some()
    }
}

struct SynthSlotSchedule {
    slots: [usize; INSTRUMENT_SLOT_COUNT],
    voice_counts: [usize; INSTRUMENT_SLOT_COUNT],
    len: usize,
}

impl SynthSlotSchedule {
    fn from_inputs(
        frames: usize,
        inputs: &[SynthSlotRenderInput; INSTRUMENT_SLOT_COUNT],
    ) -> Option<Self> {
        if frames < MIN_PARALLEL_SYNTH_FRAMES {
            return None;
        }
        let mut schedule = Self {
            slots: [0; INSTRUMENT_SLOT_COUNT],
            voice_counts: [0; INSTRUMENT_SLOT_COUNT],
            len: 0,
        };
        let mut total_voices = 0;
        for (slot, input) in inputs.iter().enumerate() {
            if !input.active {
                continue;
            }
            let voices = input.voices.iter().filter(|voice| voice.active).count();
            if voices == 0 {
                continue;
            }
            total_voices += voices;
            schedule.insert(slot, voices);
        }
        (schedule.len >= MIN_PARALLEL_SYNTH_SLOTS && total_voices >= MIN_PARALLEL_SYNTH_VOICES)
            .then_some(schedule)
    }

    fn insert(&mut self, slot: usize, voices: usize) {
        let mut index = self.len;
        while index > 0 && voices > self.voice_counts[index - 1] {
            self.slots[index] = self.slots[index - 1];
            self.voice_counts[index] = self.voice_counts[index - 1];
            index -= 1;
        }
        self.slots[index] = slot;
        self.voice_counts[index] = voices;
        self.len += 1;
    }
}

impl Drop for SynthSlotWorkerPool {
    fn drop(&mut self) {
        stop_workers(&mut self.workers);
    }
}

fn stop_workers(workers: &mut [SynthSlotWorker]) {
    for worker in workers.iter() {
        let (lock, condvar) = &*worker.shared;
        if let Ok(mut state) = lock.lock() {
            state.shutdown = true;
            condvar.notify_one();
        }
    }
    for worker in workers.iter_mut() {
        if let Some(handle) = worker.handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct SynthSlotRenderInput {
    pub active: bool,
    pub voices: [Voice; VOICES_PER_SLOT],
    pub config: SynthConfig,
    pub render_config: SynthVoiceRenderConfig,
    pub revision: u32,
    pub mods: InstrumentMod,
}

impl SynthSlotJob {
    fn from_input(
        slot: usize,
        frames: usize,
        base_sample_clock: u64,
        sample_rate: u32,
        input: SynthSlotRenderInput,
    ) -> Self {
        Self {
            slot,
            frames,
            base_sample_clock,
            sample_rate,
            active: input.active,
            voices: input.voices,
            config: input.config,
            render_config: input.render_config,
            revision: input.revision,
            mods: input.mods,
        }
    }
}

impl WorkerState {
    fn new() -> Self {
        Self {
            jobs: std::array::from_fn(|_| None),
            samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            voices: std::array::from_fn(|_| None),
            final_active: [false; INSTRUMENT_SLOT_COUNT],
            running: false,
            complete: false,
            shutdown: false,
        }
    }

    fn reset(&mut self) {
        self.jobs.fill(None);
        self.voices.fill(None);
        self.final_active.fill(false);
        self.complete = false;
    }
}

fn worker_loop(shared: Arc<(Mutex<WorkerState>, Condvar)>) {
    let (lock, condvar) = &*shared;
    loop {
        let mut state = match lock.lock() {
            Ok(state) => state,
            Err(_) => return,
        };
        while !state.running && !state.shutdown {
            state = match condvar.wait(state) {
                Ok(state) => state,
                Err(_) => return,
            };
        }
        if state.shutdown {
            return;
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            if let Some(job) = state.jobs[slot] {
                let result = {
                    let WorkerState {
                        samples, active, ..
                    } = &mut *state;
                    render_synth_slot_block(job, &mut samples[slot], &mut active[slot])
                };
                state.voices[slot] = Some(result.0);
                state.final_active[slot] = result.1;
            }
        }
        state.running = false;
        state.complete = true;
        condvar.notify_one();
    }
}

fn render_synth_slot_block(
    job: SynthSlotJob,
    samples: &mut [f32],
    active: &mut [bool],
) -> ([Voice; VOICES_PER_SLOT], bool) {
    samples[..job.frames].fill(0.0);
    active[..job.frames].fill(false);
    let mut voices = job.voices;
    if !job.active {
        return (voices, false);
    }
    for frame in 0..job.frames {
        let rendered = render_synth_slot_pool_frame(
            &mut voices,
            job.slot,
            job.base_sample_clock.saturating_add(frame as u64),
            SynthSlotFrameContext {
                sample_rate: job.sample_rate,
                config: job.config,
                render_config: &job.render_config,
                revision: job.revision,
                mods: job.mods,
            },
        );
        samples[frame] = rendered.sample;
        active[frame] = rendered.active;
    }
    (voices, voices.iter().any(|voice| voice.active))
}

pub(super) struct SynthSlotFrameContext<'a> {
    pub sample_rate: u32,
    pub config: SynthConfig,
    pub render_config: &'a SynthVoiceRenderConfig,
    pub revision: u32,
    pub mods: InstrumentMod,
}

pub(super) fn render_synth_slot_pool_frame(
    voices: &mut [Voice; VOICES_PER_SLOT],
    slot_idx: usize,
    frame_sample_clock: u64,
    context: SynthSlotFrameContext<'_>,
) -> SlotFrameOutput {
    let mut out = 0.0;
    let mut slot_active = false;
    for voice in voices.iter_mut() {
        if !voice.active {
            continue;
        }
        debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
        if frame_sample_clock >= voice.note_off_sample {
            voice
                .amp_env
                .begin_release(context.config.amp_env, context.sample_rate);
            voice
                .filt_env
                .begin_release(context.config.filter_env, context.sample_rate);
        }
        let amp_env = voice.amp_env.next();
        let filt_env = voice.filt_env.next();
        if voice.amp_env.is_off() {
            voice.active = false;
            continue;
        }
        if voice.render_revision != context.revision {
            refresh_synth_voice_render_cache(
                voice,
                context.render_config,
                context.sample_rate,
                context.revision,
            );
        }
        out += render_synth_voice_sample_precomputed(
            context.sample_rate,
            context.mods,
            context.render_config,
            voice,
            amp_env,
            filt_env,
        );
        slot_active = true;
    }
    SlotFrameOutput {
        sample: out,
        active: slot_active,
    }
}
