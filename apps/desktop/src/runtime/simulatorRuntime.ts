import { type DeviceInput, type RuntimeHostMessage, type RuntimeRunnerMessage, type RuntimeSnapshot, type RuntimeStatus } from "@octessera/device-contracts";
import { TauriAudioLoadService, type AudioLoadService, type AudioLoadStatus } from "../audio/audioLoadEvents";
import { createIntervalRuntimeScheduler, type RuntimeScheduler } from "./runtimeScheduler";
import { tauriCoreRunner } from "./runner/tauriCoreRunner";
import { applyTransientIndicatorPulse, createTransientIndicators, resetTransientIndicators, type IndicatorTimer } from "./simulatorRuntimeIndicators";
import { createInitialRuntimeSnapshot, createRuntimeSnapshotCache, mergeSnapshotSettings, normalizeSnapshotPixels, snapshotFromCore, type TransientIndicatorState } from "./simulatorSnapshot";
import { scheduleStartupSplashRefresh, type StartupSplashTimer } from "./simulatorStartupSplash";
import type { InputAction, RuntimeListener, SimulatorSnapshot } from "./types";

type SimulatorRuntime = {
  dispatch(input: DeviceInput): void;
  dispatchAction(action: InputAction): void;
  start(): void;
  stop(): void;
  subscribe(listener: RuntimeListener): () => void;
  getSnapshot(): SimulatorSnapshot;
};

type EncoderTurnInput = Extract<DeviceInput, { type: "encoder_turn" }>;
type EncoderId = NonNullable<EncoderTurnInput["id"]>;
type DesktopRunnerMessage = RuntimeRunnerMessage;

type RuntimeDeps = {
  audioLoadService?: AudioLoadService;
  runtimeDispatch?: (message: RuntimeHostMessage) => Promise<RuntimeRunnerMessage[]>;
};

export function shouldApplyRuntimeBatch(
  lastSeq: number,
  seq: number,
  nowMs: number,
  ignoreUntilMs: number,
  messages: RuntimeRunnerMessage[],
): boolean {
  return seq > lastSeq && (nowMs >= ignoreUntilMs || batchContainsFault(messages));
}

function batchContainsFault(messages: RuntimeRunnerMessage[]) {
  return messages.some((message) =>
    (message.type === "runtime_status" && message.status.error !== undefined) ||
    (message.type === "snapshot" && message.snapshot.runtimeError !== undefined)
  );
}

const TAURI_DISPLAY_DRAIN_MS = 66;
const ASYNC_RUNTIME_SUPPRESS_MS = 120;

export function createSimulatorRuntime(scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8), deps: RuntimeDeps = {}): SimulatorRuntime {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const runtimeDispatch = deps.runtimeDispatch ?? (isTauri ? (message: RuntimeHostMessage) => tauriCoreRunner.dispatchRuntime(message) : null);
  if (!runtimeDispatch) {
    throw new Error("Desktop runtime requires Tauri native runtime or an injected native dispatch");
  }
  const dispatchRuntime = runtimeDispatch;

  let latestFrame: RuntimeSnapshot = createInitialRuntimeSnapshot();
  let shiftActive = false;
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };
  let runtimeStatus: RuntimeStatus | null = null;
  let runtimeUpdateEpoch = 0;
  let lastAsyncRuntimeSeq = 0;
  let lastTauriDrainAt = 0;
  let tauriDrainInFlight = false;
  let ignoreAsyncUntilMs = 0;
  const snapshotCache = createRuntimeSnapshotCache();
  const pendingEncoderTurns: Array<{ id: EncoderId; delta: number }> = [];
  let pendingEncoderTimer: ReturnType<typeof setTimeout> | null = null;
  let startupSplashTimer: StartupSplashTimer = null;
  let indicatorTimer: IndicatorTimer = null;
  const indicators: TransientIndicatorState = createTransientIndicators();
  const queuedRuntimeMessages: RuntimeHostMessage[] = [];
  let runtimeDispatchInFlight = false;
  const listeners = new Set<RuntimeListener>();
  const audioLoadService = deps.audioLoadService ?? new TauriAudioLoadService();

  if (isTauri && !deps.runtimeDispatch) {
    void tauriCoreRunner.listenRuntimeMessages((batch) => {
      applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now());
    }).catch((err) => {
      console.error("[Runtime] listenRuntimeMessages failed:", err);
      console.error("[Runtime] listenRuntimeMessages failed:", err);
    });
  }

  void audioLoadService.listenAudioLoad((status) => {
    audioLoad = { ratio: Math.max(0, Math.min(2, status.ratio)), voiceSteal: status.voiceSteal };
    publishSnapshot();
  });

  function publishSnapshot() {
    const snapshot = snapshotFromCore(latestFrame, snapshotCache, shiftActive, indicators, { audioLoad, runtimeStatus });
    for (const listener of listeners) listener(snapshot);
  }

  function processRunnerMessages(messages: DesktopRunnerMessage[]) {
    let hasSnapshot = false;
    for (const message of messages) {
      hasSnapshot ||= message.type === "snapshot";
      processRunnerMessage(message);
    }
    return hasSnapshot;
  }

  function processRunnerMessage(message: DesktopRunnerMessage) {
    if (message.type === "snapshot") {
      applySnapshotMessage(message.snapshot);
      return;
    }
    if (message.type === "ui_pulse") {
      applyUiPulse(message.pulse);
      return;
    }
    if (message.type === "runtime_status") {
      runtimeStatus = message.status;
      return;
    }
  }

  function applySnapshotMessage(snapshot: RuntimeSnapshot) {
    normalizeSnapshotPixels(snapshot);
    mergeSnapshotSettings(snapshot, latestFrame);
    latestFrame = snapshot;
    startupSplashTimer = scheduleStartupSplashRefresh(snapshot, startupSplashTimer, mirrorRuntimeMessage, () => {
      startupSplashTimer = null;
    });
  }

  function applyUiPulse(pulse: Extract<RuntimeRunnerMessage, { type: "ui_pulse" }>['pulse']) {
    indicatorTimer = applyTransientIndicatorPulse(pulse, indicators, indicatorTimer, publishSnapshot, () => {
      indicatorTimer = null;
      publishSnapshot();
    });
  }

  function drainQueuedRuntimeMessages() {
    if (runtimeDispatchInFlight) return;
    const message = queuedRuntimeMessages.shift();
    if (!message) {
      if (pendingEncoderTurns.length > 0 && pendingEncoderTimer === null) {
        flushPendingEncoderTurns();
      }
      return;
    }
    runtimeUpdateEpoch += 1;
    ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
    runtimeDispatchInFlight = true;
    void dispatchRuntime(message)
      .then((messages) => {
        if (!processRunnerMessages(messages)) ignoreAsyncUntilMs = 0;
        publishSnapshot();
      })
      .catch((err) => {
        console.error("[Runtime] runtimeDispatch failed:", err);
        publishSnapshot();
      })
      .finally(() => {
        runtimeDispatchInFlight = false;
        drainQueuedRuntimeMessages();
      });
  }

  function mirrorRuntimeMessage(message: RuntimeHostMessage) {
    queuedRuntimeMessages.push(message);
    drainQueuedRuntimeMessages();
  }

  function applyAsyncRuntimeBatch(seq: number, messages: RuntimeRunnerMessage[], nowMs: number) {
    if (!shouldApplyRuntimeBatch(lastAsyncRuntimeSeq, seq, nowMs, ignoreAsyncUntilMs, messages)) return;
    lastAsyncRuntimeSeq = seq;
    processRunnerMessages(messages);
    publishSnapshot();
  }

  function maybeDrainTauriRuntimeMessages(nowMs: number) {
    if (!isTauri || deps.runtimeDispatch || tauriDrainInFlight) return;
    if (nowMs - lastTauriDrainAt < TAURI_DISPLAY_DRAIN_MS) return;
    tauriDrainInFlight = true;
    lastTauriDrainAt = nowMs;
    void tauriCoreRunner.drainRuntimeMessages()
      .then((batches) => {
        for (const batch of batches) applyAsyncRuntimeBatch(batch.seq, batch.messages, nowMs);
      })
      .catch((err) => console.error("[Runtime] drainRuntimeMessages failed:", err))
      .finally(() => {
        tauriDrainInFlight = false;
      });
  }

  function dispatchToRunner(input: DeviceInput) {
    flushPendingEncoderTurns(true);
    mirrorRuntimeMessage({ type: "device_input", input });
  }

  function flushPendingEncoderTurns(forceQueue = false) {
    if (pendingEncoderTimer !== null) {
      clearTimeout(pendingEncoderTimer);
      pendingEncoderTimer = null;
    }
    if (!forceQueue && (runtimeDispatchInFlight || queuedRuntimeMessages.length > 0)) {
      pendingEncoderTimer = setTimeout(() => flushPendingEncoderTurns(), 8);
      return;
    }
    const turns = pendingEncoderTurns.splice(0);
    for (const { id, delta } of turns) {
      if (delta === 0) continue;
      mirrorRuntimeMessage({ type: "device_input", input: { type: "encoder_turn", id, delta } });
    }
  }

  function dispatchEncoderTurn(input: EncoderTurnInput) {
    const id = input.id ?? "main";
    const last = pendingEncoderTurns.at(-1);
    if (last && last.id === id && Math.sign(last.delta) === Math.sign(input.delta)) {
      last.delta = Math.max(-127, Math.min(127, last.delta + input.delta));
    } else {
      pendingEncoderTurns.push({ id, delta: Math.max(-127, Math.min(127, input.delta)) });
    }
    if (pendingEncoderTimer !== null) return;
    pendingEncoderTimer = setTimeout(flushPendingEncoderTurns, 8);
  }

  return {
    dispatch(input) {
      if (input.type === "encoder_turn") {
        dispatchEncoderTurn(input);
      } else {
        dispatchToRunner(input);
      }
    },
    dispatchAction(action) {
      dispatchInputAction(action);
    },
    start() {
      runtimeUpdateEpoch += 1;
      ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
      void dispatchRuntime({ type: "transport_pulse_step", pulses: 0, source: "internal", requestSnapshot: true })
        .then((messages) => {
          processRunnerMessages(messages);
          publishSnapshot();
        })
        .catch((err) => {
          console.error("[Runtime] initial pulse_step failed:", err);
          publishSnapshot();
        });
      scheduler.start((nowMs) => {
        maybeDrainTauriRuntimeMessages(nowMs);
      });
      publishSnapshot();
    },
    stop() {
      flushPendingEncoderTurns(true);
      if (startupSplashTimer !== null) {
        clearTimeout(startupSplashTimer);
        startupSplashTimer = null;
      }
      if (indicatorTimer !== null) {
        clearTimeout(indicatorTimer);
        indicatorTimer = null;
      }
      resetTransientIndicators(indicators);
      publishSnapshot();
      scheduler.stop();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshotFromCore(latestFrame, snapshotCache, shiftActive, indicators, { audioLoad, runtimeStatus }));
      return () => listeners.delete(listener);
    },
    getSnapshot() {
      return snapshotFromCore(latestFrame, snapshotCache, shiftActive, indicators, { audioLoad, runtimeStatus });
    }
  };

  function dispatchInputAction(action: InputAction) {
    if (action.type === "emergency_brake") {
      dispatchToRunner({ type: "button_s", pressed: true });
      return;
    }
    if (action.type === "shift") {
      shiftActive = action.active;
      dispatchToRunner({ type: "button_shift", pressed: action.active });
      return;
    }
    if (action.type === "fn") {
      dispatchToRunner({ type: "button_fn", pressed: action.active });
      return;
    }
    if (action.input.type === "encoder_turn") {
      dispatchEncoderTurn(action.input);
    } else {
      dispatchToRunner(action.input);
    }
  }
}
