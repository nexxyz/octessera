# Parallel DSP Plan

This is the pre-parallelization plan for using multiple Pi Zero 2 W cores in the realtime audio engine without making the audio callback fragile.

## Goal

Parallelize only the parts of the DSP graph that are naturally independent: per-instrument rendering and, later, per-FX-bus processing. Keep master FX and final mix serial until profiling proves otherwise.

## Phase 0: Measure first

Add per-stage timing around:

- synth voice render;
- sample voice render;
- preview sample render;
- slot-to-bus accumulation;
- FX bus processing;
- master FX and final mix.

Use representative idle, heavy synth, heavy sampler, and heavy FX patches on PC and Pi. Record baseline CPU, p95/max block time, xrun/error logs, and USB/physical output behavior at the current 512-frame buffer.

## Phase 1: Serial render graph refactor

Refactor the current block render into an explicit serial graph with no behavior change:

1. apply control events at the block boundary;
2. render instrument/slot outputs;
3. accumulate slots into buses;
4. process FX buses;
5. apply master FX/final mix;
6. interleave output.

Buffers and scratch space must be preallocated and named. This phase should produce near/bit-equivalent audio output and pass the existing render/routing/voice tests.

## Phase 2: Parallel per-slot rendering

Introduce an opt-in persistent worker pool created outside the audio callback. No per-buffer thread spawning, allocation, logging, or unbounded queues.

First parallel target:

- synth instrument slots;
- sampler instrument slots;
- preview/sample voices where ownership is isolated.

Rules:

- each worker mutates only its assigned slot state;
- shared config/sample data is immutable for the block;
- control events are applied before dispatch;
- merge order stays serial and deterministic;
- fallback is decided before dispatching a block, never after workers have mutated state.

## Phase 3: Parallel FX buses

After deterministic slot-to-bus accumulation, process independent FX buses in parallel. Per-bus FX state may be mutated by only that bus worker.

Keep serial:

- slot-to-bus accumulation;
- master FX;
- final stereo mix.

## Phase 4: Pi scheduling policy

Add explicit worker scheduling policy before live Pi use:

- audio callback keeps highest priority;
- DSP workers are persistent and lower/equal priority as appropriate;
- runtime/render/input threads must not preempt audio work;
- optional affinity experiments can pin audio away from UI/input/render.

## Phase 5: Deadline safety

Worker miss policy must be state-safe:

- no mid-block fallback after partial state mutation;
- if workers miss a deadline, complete or drop at a clean boundary;
- mark parallel mode unhealthy and return to the serial path for future blocks;
- expose telemetry for misses and fallback.

## Validation

Required before enabling by default:

- near/bit-equivalence against serial rendering for deterministic patches;
- stress tests with many synth voices, sampler voices, and FX buses;
- Pi live tests at 512 frames on physical and USB output;
- no xrun/POLLERR/log floods;
- CPU improves under heavy load and does not regress idle load.

## First approved milestone

Only Phase 0 and Phase 1 are approved as the first implementation milestone: serial render graph plus instrumentation, no parallel execution yet.
