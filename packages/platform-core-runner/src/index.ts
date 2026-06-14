import { getBehavior } from "@cellsymphony/behavior-api";
import type {
  RuntimeAudioCommand,
  RuntimeHostMessage,
  RuntimePlatformEffect,
  RuntimeRunnerMessage,
  RuntimeStatus,
  RuntimeStoreResult
} from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import {
  createInitialState,
  applyStoreResult,
  routeInput,
  stepTransportByPulses,
  toSimulatorFrame,
  type PlatformEffect,
  type PlatformState,
  type StoreResult
} from "@cellsymphony/platform-core";

export type CoreRunner = {
  dispatch(message: RuntimeHostMessage): RuntimeRunnerMessage[];
  getFrame(): ReturnType<typeof toSimulatorFrame>;
  getState(): PlatformState<any>;
  getStatus(): RuntimeStatus;
};

export type CoreRunnerOptions = {
  initialBehaviorId?: string;
};

export function createCoreRunner(options: CoreRunnerOptions = {}): CoreRunner {
  const initialBehaviorId = options.initialBehaviorId ?? "life";
  const initialBehavior = getBehavior(initialBehaviorId) ?? getBehavior("life");
  if (!initialBehavior) throw new Error("No default behavior registered for core runner");

  let state = createInitialState(initialBehavior);

  const activeBehavior = () => getBehavior(String(state.runtimeConfig.activeBehavior)) ?? getBehavior(state.activeBehavior) ?? initialBehavior;

  const snapshotMessage = (): RuntimeRunnerMessage => ({
    type: "snapshot",
    snapshot: toSimulatorFrame(state, activeBehavior())
  });

  const statusMessage = (): RuntimeRunnerMessage => ({
    type: "runtime_status",
    status: currentStatus(state)
  });

  return {
    dispatch(message) {
      const behavior = activeBehavior();
      let events: MusicalEvent[] = [];
      let effects = [] as PlatformEffect[];

      if (message.type === "device_input") {
        const result = routeInput(state, message.input, behavior);
        state = result.state;
        events = result.events;
        effects = result.effects;
      } else if (message.type === "transport_pulse_step") {
        const result = stepTransportByPulses(state, behavior, message.pulses, message.source);
        state = result.state;
        events = result.events;
      } else if (message.type === "midi_realtime") {
        const input = message.message === "clock"
          ? { type: "midi_clock", pulses: message.pulses }
          : { type: message.message === "start" ? "midi_start" : message.message === "continue" ? "midi_continue" : "midi_stop" };
        const result = routeInput(state, input as any, behavior);
        state = result.state;
        events = result.events;
        effects = result.effects;
      } else {
        const result = applyStoreResult(state, message.result as StoreResult, behavior);
        state = result.state;
        effects = result.effects;
      }

      const out: RuntimeRunnerMessage[] = [];
      if (effects.length > 0) out.push({ type: "platform_effects", effects: effects.map(toRuntimePlatformEffect) });

      const audioCommands = effects
        .filter((effect): effect is Extract<PlatformEffect, { type: "audio_command" }> => effect.type === "audio_command")
        .map((effect) => effect.command as RuntimeAudioCommand);
      if (audioCommands.length > 0) out.push({ type: "audio_commands", commands: audioCommands });

      if (events.length > 0) out.push({ type: "musical_events", events });
      if (message.type !== "transport_pulse_step" || message.requestSnapshot !== false) {
        out.push(snapshotMessage());
      }
      out.push(statusMessage());
      return out;
    },
    getFrame() {
      return toSimulatorFrame(state, activeBehavior());
    },
    getState() {
      return state;
    },
    getStatus() {
      return currentStatus(state);
    }
  };
}

function currentStatus(state: PlatformState<any>): RuntimeStatus {
  return {
    state: state.transport.playing ? "running" : state.system.pausedByUser ? "paused" : "idle",
    transport: state.transport.playing ? "playing" : state.system.stopLatched ? "stopped" : "paused",
    currentPpqnPulse: state.transport.ppqnPulse,
    pendingResync: state.system.pendingResync,
    syncSource: state.runtimeConfig.midi.syncMode === "external" ? "external" : "internal"
  };
}

function toRuntimePlatformEffect(effect: PlatformEffect): RuntimePlatformEffect {
  return effect as unknown as RuntimePlatformEffect;
}

export function toStoreResult(result: RuntimeStoreResult): StoreResult {
  return result as StoreResult;
}
