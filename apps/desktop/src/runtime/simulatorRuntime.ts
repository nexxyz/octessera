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
  let cachedAudioRevision: number | undefined;
  let cachedInstruments: unknown[] = [];
  let cachedMixer: unknown = { buses: [] };
  let cachedPanPositions: number = PAN_POSITION_COUNT;
  let cachedMasterVolume: number = 100;
  const pendingEncoderTurns = new Map<string, number>();
  let pendingEncoderTimer: ReturnType<typeof setTimeout> | null = null;
  let startupSplashTimer: ReturnType<typeof setTimeout> | null = null;
  let indicatorTimer: ReturnType<typeof setTimeout> | null = null;
  let eventDotUntilMs = 0;
  let transportFlashUntilMs = 0;
  let transientTransportFlash: "measure" | "beat" | "none" = "none";
  const queuedRuntimeMessages: RuntimeHostMessage[] = [];
  let runtimeDispatchInFlight = false;
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
    const audioRevision = settings?.audioConfigRevision;
    if (settings && (cachedAudioRevision === undefined || audioRevision === undefined || audioRevision !== cachedAudioRevision)) {
      cachedAudioRevision = audioRevision;
      cachedInstruments = settings.instruments ?? [];
      cachedMixer = settings.mixer ?? { buses: [] };
      cachedPanPositions = settings.panPositions ?? PAN_POSITION_COUNT;
      cachedMasterVolume = settings.masterVolume ?? 100;
    }
    const flash = performance.now() < transportFlashUntilMs
      ? transientTransportFlash
      : String((frame as any).transportFlash ?? "none");
    const combined = settings?.combinedModifierHeld ?? false;
    return {
      frame: withTransientIndicators(frame),
      neoKeyLeds: {
        back: "solid_red",
        space: !frame.transport.playing ? "off" : flash === "measure" ? "measure" : flash === "beat" ? "beat" : "off",
        shift: combined ? "solid_blue" : (settings?.shiftHeld ?? shiftActive) ? "solid_yellow" : "off",
        fn: combined ? "solid_blue" : (settings?.fnHeld ?? false) ? "solid_yellow" : "off"
      },
      displayBrightness: settings?.displayBrightness ?? 75,
      buttonBrightness: settings?.buttonBrightness ?? 75,
      masterVolume: cachedMasterVolume,
      voiceStealingMode: settings?.voiceStealingMode ?? "auto-balanced",
      audioLoad,
      audioError,
      instruments: cachedInstruments,
      mixer: cachedMixer,
      panPositions: cachedPanPositions,
      audioConfigRevision: cachedAudioRevision,
      autoSaveFlash: settings?.autoSaveFlash ?? "none",
      autoSaveFlashSerial: settings?.autoSaveFlashSerial
    };
  }

  function withTransientIndicators(frame: RuntimeSnapshot): RuntimeSnapshot {
    const transientEventDotOn = performance.now() < eventDotUntilMs;
    const transientTransport = performance.now() < transportFlashUntilMs ? transientTransportFlash : null;
    if (!transientEventDotOn && transientTransport === null) return frame;
    return {
      ...(frame as any),
      ...(transientEventDotOn ? { eventDotOn: true } : {}),
      ...(transientTransport ? { transportFlash: transientTransport } : {}),
    } as RuntimeSnapshot;
  }

  function publishSnapshot() {
    const snapshot = snapshotFromCore(latestFrame);
    for (const listener of listeners) listener(snapshot);
  }

  function syncPlaybackConfigIfNeeded() {
    if (deps.runtimeDispatch) return;
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
      processRunnerMessage(message);
    }
  }

  function processRunnerMessage(message: RuntimeRunnerMessage) {
    if (message.type === "snapshot") {
      applySnapshotMessage(message.snapshot);
      return;
    }
    if (message.type === "ui_pulse") {
      applyUiPulse(message.pulse);
      return;
    }
    if ((message as any).type === "audio_error") {
      applyAudioErrorMessage((message as any).error ?? null);
    }
  }

  function applySnapshotMessage(snapshot: RuntimeSnapshot) {
    normalizeSnapshotPixels(snapshot);
    mergeSnapshotSettings(snapshot);
    latestFrame = snapshot;
    scheduleStartupSplashRefresh(snapshot);
  }

  function normalizeSnapshotPixels(snapshot: RuntimeSnapshot) {
    if (snapshot.oled && !(snapshot.oled.pixels instanceof Uint8Array)) {
      snapshot.oled = { ...snapshot.oled, pixels: new Uint8Array(Object.values(snapshot.oled.pixels as any)) };
    }
  }

  function mergeSnapshotSettings(snapshot: RuntimeSnapshot) {
    const previousSettings = latestFrame.settings;
    const nextSettings = snapshot.settings;
    if (!previousSettings || !nextSettings) return;
    if (!("instruments" in nextSettings)) nextSettings.instruments = previousSettings.instruments;
    if (!("mixer" in nextSettings)) nextSettings.mixer = previousSettings.mixer;
    if (!("panPositions" in nextSettings)) nextSettings.panPositions = previousSettings.panPositions;
  }

  function applyAudioErrorMessage(error: string | null) {
    audioError = error;
    if (audioError) console.warn("[Runtime] audio error:", audioError);
  }

  function applyUiPulse(pulse: Extract<RuntimeRunnerMessage, { type: "ui_pulse" }>['pulse']) {
    const now = performance.now();
    if (pulse.type === "trigger_pulse") {
      eventDotUntilMs = now + pulse.durationMs;
    } else if (pulse.type === "transport_flash") {
      transientTransportFlash = pulse.flash;
      transportFlashUntilMs = now + pulse.durationMs;
    }
    if (indicatorTimer !== null) clearTimeout(indicatorTimer);
    publishSnapshot();
    const nextUntil = Math.max(eventDotUntilMs, transportFlashUntilMs);
    indicatorTimer = setTimeout(() => {
      indicatorTimer = null;
      publishSnapshot();
    }, Math.max(0, nextUntil - now) + 5);
  }

  function scheduleStartupSplashRefresh(snapshot: RuntimeSnapshot) {
    const splash = String((snapshot.display as any)?.splash ?? "");
    if (splash !== "startup") {
      if (startupSplashTimer !== null) {
        clearTimeout(startupSplashTimer);
        startupSplashTimer = null;
      }
      return;
    }
    if (startupSplashTimer !== null) return;
    startupSplashTimer = setTimeout(() => {
      startupSplashTimer = null;
      mirrorRuntimeMessage({
        type: "transport_pulse_step",
        pulses: 0,
        source: "internal",
        requestSnapshot: true,
      });
    }, 1600);
  }

  function drainQueuedRuntimeMessages() {
    if (runtimeDispatchInFlight) return;
    const message = queuedRuntimeMessages.shift();
    if (!message) {
      if (pendingEncoderTurns.size > 0 && pendingEncoderTimer === null) {
        flushPendingEncoderTurns();
      }
      return;
    }
    runtimeUpdateEpoch += 1;
    ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
    runtimeDispatchInFlight = true;
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
    flushPendingEncoderTurns(true);
    syncPlaybackConfigIfNeeded();
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
    for (const [id, delta] of pendingEncoderTurns) {
      pendingEncoderTurns.delete(id);
      if (delta === 0) continue;
      syncPlaybackConfigIfNeeded();
      mirrorRuntimeMessage({ type: "device_input", input: { type: "encoder_turn", id: id as any, delta } as DeviceInput });
    }
  }

  function dispatchEncoderTurn(input: Extract<DeviceInput, { type: "encoder_turn" }>) {
    const id = input.id ?? "main";
    pendingEncoderTurns.set(id, Math.max(-127, Math.min(127, (pendingEncoderTurns.get(id) ?? 0) + input.delta)));
    if (pendingEncoderTimer !== null) return;
    pendingEncoderTimer = setTimeout(flushPendingEncoderTurns, 8);
  }

  syncPlaybackConfigIfNeeded();

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
          audioError = `initial pulse_step failed: ${err instanceof Error ? err.message : String(err)}`;
          publishSnapshot();
        });
      scheduler.start((nowMs) => {
        syncPlaybackConfigIfNeeded();
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
      eventDotUntilMs = 0;
      transportFlashUntilMs = 0;
      transientTransportFlash = "none";
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
