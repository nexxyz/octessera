import { PAN_POSITION_COUNT, type DeviceInput, type OledFrame, type RuntimeHostMessage, type RuntimeRunnerMessage, type RuntimeSnapshot } from "@cellsymphony/device-contracts";
import { TauriAudioLoadService, type AudioLoadService, type AudioLoadStatus } from "../audio/audioLoadEvents";
import { createIntervalRuntimeScheduler, type RuntimeScheduler } from "./runtimeScheduler";
import { TauriMidiService } from "./midi/tauriMidi";
import { tauriCoreRunner } from "./runner/tauriCoreRunner";
import type { InputAction, RuntimeListener, SimulatorSnapshot } from "./types";

type SimulatorRuntime = {
  dispatch(input: DeviceInput): void;
  dispatchAction(action: InputAction): void;
  start(): void;
  stop(): void;
  subscribe(listener: RuntimeListener): () => void;
  getSnapshot(): SimulatorSnapshot;
};

type RuntimeMidiService = {
  listenMidiIn(handler: (bytes: Uint8Array) => void): Promise<() => void>;
};

type RuntimeDeps = {
  midiService?: RuntimeMidiService;
  audioLoadService?: AudioLoadService;
  runtimeDispatch?: (message: RuntimeHostMessage) => Promise<RuntimeRunnerMessage[]>;
};

const TAURI_DISPLAY_DRAIN_MS = 66;
const ASYNC_RUNTIME_SUPPRESS_MS = 120;

export function createSimulatorRuntime(scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8), deps: RuntimeDeps = {}): SimulatorRuntime {
  const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const runtimeDispatch = deps.runtimeDispatch ?? (isTauri ? (message: RuntimeHostMessage) => tauriCoreRunner.dispatchRuntime(message) : null);
  if (!runtimeDispatch) {
    throw new Error("Desktop runtime requires Tauri native runtime or an injected native dispatch");
  }
  const dispatchRuntime = runtimeDispatch;

  const blankOled: OledFrame = { width: 128, height: 128, format: "rgb565be", pixels: new Uint8Array(32768) };
  let latestFrame: RuntimeSnapshot = {
    oled: blankOled,
    leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
    transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
    display: { page: "boot", title: "Boot", lines: [], editing: false },
    activeBehavior: "life",
    gridInteraction: "paint"
  };
  let shiftActive = false;
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };
  let audioError: string | null = null;
  let lastSyncedPlaybackConfig = "";
  let runtimeUpdateEpoch = 0;
  let lastAsyncRuntimeSeq = 0;
  let lastTauriDrainAt = 0;
  let tauriDrainInFlight = false;
  let ignoreAsyncUntilMs = 0;
  const listeners = new Set<RuntimeListener>();
  const tauriMidi = deps.midiService ?? (isTauri && !deps.runtimeDispatch ? new TauriMidiService() : null);
  const audioLoadService = deps.audioLoadService ?? new TauriAudioLoadService();

  if (isTauri && !deps.runtimeDispatch) {
    void tauriCoreRunner.listenRuntimeMessages((batch) => {
      applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now());
    }).catch((err) => {
      console.error("[Runtime] listenRuntimeMessages failed:", err);
      audioError = `runtime messages failed: ${err instanceof Error ? err.message : String(err)}`;
      publishSnapshot();
    });
  }

  void tauriMidi?.listenMidiIn((data: Uint8Array) => {
    if (!isTauri || deps.runtimeDispatch) return;
    void tauriCoreRunner.handleMidiRealtime(data)
      .then((messages) => {
        processRunnerMessages(messages);
        publishSnapshot();
      })
      .catch((err) => {
        console.error("[Runtime] handleMidiRealtime failed:", err);
      });
  });

  void audioLoadService.listenAudioLoad((status) => {
    audioLoad = { ratio: Math.max(0, Math.min(2, status.ratio)), voiceSteal: status.voiceSteal };
    publishSnapshot();
  });

  function frameSettings() {
    return latestFrame.settings;
  }

  function snapshotFromCore(frame: RuntimeSnapshot): SimulatorSnapshot {
    const settings = frame.settings;
    const flash = settings?.transportFlash ?? "none";
    const combined = settings?.combinedModifierHeld ?? false;
    return {
      frame,
      neoKeyLeds: {
        back: "solid_red",
        space: !frame.transport.playing ? "off" : flash === "measure" ? "measure" : flash === "beat" ? "beat" : "off",
        shift: combined ? "solid_blue" : (settings?.shiftHeld ?? shiftActive) ? "solid_yellow" : "off",
        fn: combined ? "solid_blue" : (settings?.fnHeld ?? false) ? "solid_yellow" : "off"
      },
      displayBrightness: settings?.displayBrightness ?? 75,
      buttonBrightness: settings?.buttonBrightness ?? 75,
      masterVolume: settings?.masterVolume ?? 100,
      voiceStealingMode: settings?.voiceStealingMode ?? "balanced",
      audioLoad,
      audioError,
      instruments: settings?.instruments ?? [],
      mixer: settings?.mixer ?? { buses: [] },
      panPositions: settings?.panPositions ?? PAN_POSITION_COUNT,
      autoSaveFlash: settings?.autoSaveFlash ?? "none",
      autoSaveFlashSerial: settings?.autoSaveFlashSerial
    };
  }

  function publishSnapshot() {
    const snapshot = snapshotFromCore(latestFrame);
    for (const listener of listeners) listener(snapshot);
  }

  function syncPlaybackConfigIfNeeded() {
    const settings = frameSettings();
    const midi = settings?.midi;
    if (!midi) return;
    const config = {
      bpm: Number(latestFrame.transport.bpm ?? 120),
      syncSource: midi.syncMode === "external" ? "external" : "internal",
      midiClockOutEnabled: Boolean(midi.clockOutEnabled),
      midiOutEnabled: Boolean(midi.enabled && midi.outId)
    } as const;
    const signature = JSON.stringify(config);
    if (signature === lastSyncedPlaybackConfig) return;
    lastSyncedPlaybackConfig = signature;
    void tauriCoreRunner.syncConfig(config).catch((err) => {
      console.error("[Runtime] syncConfig failed:", err);
      if (lastSyncedPlaybackConfig === signature) lastSyncedPlaybackConfig = "";
    });
  }

  function processRunnerMessages(messages: RuntimeRunnerMessage[]) {
    for (const message of messages) {
      if (message.type === "snapshot") {
        const snapshot = message.snapshot;
        if (snapshot.oled && !(snapshot.oled.pixels instanceof Uint8Array)) {
          snapshot.oled = { ...snapshot.oled, pixels: new Uint8Array(Object.values(snapshot.oled.pixels as any)) };
        }
        latestFrame = snapshot;
        continue;
      }
      if ((message as any).type === "audio_error") {
        audioError = (message as any).error ?? null;
        if (audioError) console.warn("[Runtime] audio error:", audioError);
      }
    }
  }

  function mirrorRuntimeMessage(message: RuntimeHostMessage) {
    runtimeUpdateEpoch += 1;
    ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
    void dispatchRuntime(message)
      .then((messages) => {
        processRunnerMessages(messages);
        publishSnapshot();
      })
      .catch((err) => {
        console.error("[Runtime] runtimeDispatch failed:", err);
        audioError = `dispatch failed: ${err instanceof Error ? err.message : String(err)}`;
        if (latestFrame.transport.playing) {
          latestFrame = { ...latestFrame, transport: { ...latestFrame.transport, playing: false } };
        }
        publishSnapshot();
      });
  }

  function applyAsyncRuntimeBatch(seq: number, messages: RuntimeRunnerMessage[], nowMs: number) {
    if (seq <= lastAsyncRuntimeSeq) return;
    if (nowMs < ignoreAsyncUntilMs) return;
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
    syncPlaybackConfigIfNeeded();
    mirrorRuntimeMessage({ type: "device_input", input });
  }

  syncPlaybackConfigIfNeeded();

  return {
    dispatch(input) {
      dispatchToRunner(input);
    },
    dispatchAction(action) {
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
      dispatchToRunner(action.input);
    },
    start() {
      runtimeUpdateEpoch += 1;
      ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
      void dispatchRuntime({ type: "transport_pulse_step", pulses: 0, source: "internal" })
        .then((messages) => {
          processRunnerMessages(messages);
          publishSnapshot();
        })
        .catch((err) => {
          console.error("[Runtime] initial pulse_step failed:", err);
          audioError = `initial pulse_step failed: ${err instanceof Error ? err.message : String(err)}`;
          publishSnapshot();
        });
      scheduler.start((nowMs) => {
        syncPlaybackConfigIfNeeded();
        maybeDrainTauriRuntimeMessages(nowMs);
        publishSnapshot();
      });
      publishSnapshot();
    },
    stop() {
      publishSnapshot();
      scheduler.stop();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshotFromCore(latestFrame));
      return () => listeners.delete(listener);
    },
    getSnapshot() {
      return snapshotFromCore(latestFrame);
    }
  };
}
