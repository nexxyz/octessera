import type { Deps } from "./inputModifier";
import type { PlatformState, PlatformEffect } from "./index";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { EVENT_BLIP_MS, deadlineMs, nowMs } from "./timing";
import { resetScanState } from "./transportSafety";

export function handleMIDIClock<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  behavior: BehaviorEngine<TState, unknown>,
  deps: Deps<TState>,
  nextState: PlatformState<TState>,
  events: MusicalEvent[],
  _effects: PlatformEffect[]
): { state: PlatformState<TState>; events: MusicalEvent[] } {
  if (nextState.runtimeConfig.midi.syncMode === "external" && nextState.runtimeConfig.midi.clockInEnabled) {
    const pulses = Math.max(0, Math.floor((input as any).pulses ?? 0));
    const advanced = deps.applyExternalClockPulses(nextState, behavior, pulses);
    nextState = advanced.state;
    events.push(...advanced.events);
    if (advanced.events.some((e) => e.type === "note_on")) nextState.system = { ...nextState.system, eventBlipUntilMs: deadlineMs(nowMs(), EVENT_BLIP_MS) };
  }
  return { state: nextState, events };
}

export function handleMIDIStartStop<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  _behavior: BehaviorEngine<TState, unknown>,
  _deps: Deps<TState>,
  nextState: PlatformState<TState>,
  _events: MusicalEvent[],
  _effects: PlatformEffect[]
): { state: PlatformState<TState> } {
  if (nextState.runtimeConfig.midi.syncMode === "external" && nextState.runtimeConfig.midi.clockInEnabled && nextState.runtimeConfig.midi.respondToStartStop) {
    if (input.type === "midi_stop") {
      nextState.transport = { ...nextState.transport, playing: false };
      nextState.system = { ...nextState.system, stopLatched: true };
    } else if (!nextState.system.pausedByUser) {
      if (input.type === "midi_start") {
        const scanReset = resetScanState(nextState);
        nextState.transport = { ...nextState.transport, playing: true, ppqnPulse: 0, tick: 0 };
        nextState.partScanIndex = scanReset.partScanIndex;
        nextState.partScanPulseAccumulator = scanReset.partScanPulseAccumulator;
        nextState.partAlgorithmPulseAccumulator = scanReset.partAlgorithmPulseAccumulator;
        nextState.scanIndex = scanReset.scanIndex;
        nextState.scanPulseAccumulator = scanReset.scanPulseAccumulator;
        nextState.algorithmPulseAccumulator = scanReset.algorithmPulseAccumulator;
        nextState.ppqnPulseRemainder = scanReset.ppqnPulseRemainder;
        nextState.system = { ...nextState.system, stopLatched: false, pendingResync: false, externalPpqnPulse: 0 };
      } else {
        nextState.transport = { ...nextState.transport, playing: true };
        nextState.system = { ...nextState.system, stopLatched: false };
      }
    }
  }
  return { state: nextState };
}
