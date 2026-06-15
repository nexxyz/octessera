import type {
  DeviceInput,
  OledFrame,
  RuntimeAudioCommand,
  RuntimeHostMessage,
  RuntimePlatformEffect,
  RuntimeRunnerMessage,
  RuntimeStoreResult,
  RuntimeSnapshot
} from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { PAN_POSITION_COUNT, type ConfigPayload } from "@cellsymphony/platform-core";
import { createIntervalRuntimeScheduler, type RuntimeScheduler } from "./runtimeScheduler";
import type { EventsListener, InputAction, RuntimeListener, SimulatorSnapshot } from "./types";
import { createLocalStorageConfigStore } from "./configStore";
import { TauriMidiService } from "./midi/tauriMidi";
import { invoke } from "@tauri-apps/api/core";
import { TauriAudioLoadService, type AudioLoadService, type AudioLoadStatus } from "../audio/audioLoadEvents";
import { sendEventsToAudio } from "./outputAdapters/audioSink";
import { tauriCoreRunner } from "./runner/tauriCoreRunner";

type SimulatorRuntime = {
  dispatch(input: DeviceInput): void;
  dispatchAction(action: InputAction): void;
  start(): void;
  stop(): void;
  subscribe(listener: RuntimeListener): () => void;
  subscribeEvents(listener: EventsListener): () => void;
  getSnapshot(): SimulatorSnapshot;
};

type RuntimeStore = {
  listPresets(): string[];
  loadPreset(name: string): ConfigPayload | null;
  savePreset(name: string, payload: ConfigPayload): "created" | "overwritten";
  deletePreset(name: string): boolean;
  loadDefault(): ConfigPayload | null;
  saveDefault(payload: ConfigPayload): void;
};

type RuntimeMidiService = {
  listOutputs(): Promise<{ id: string; name: string }[]>;
  listInputs(): Promise<{ id: string; name: string }[]>;
  selectOutput(id: string | null): Promise<{ ok: boolean; message?: string }>;
  selectInput(id: string | null): Promise<{ ok: boolean; message?: string }>;
  send(bytes: Uint8Array): Promise<void>;
  listenMidiIn(handler: (bytes: Uint8Array) => void): Promise<() => void>;
};

type LocalRuntimeRunner = {
  dispatch(message: RuntimeHostMessage): RuntimeRunnerMessage[];
  getState(): unknown;
  getFrame(): RuntimeSnapshot;
};

type RuntimeDeps = {
  runner?: LocalRuntimeRunner;
  store?: RuntimeStore;
  midiService?: RuntimeMidiService;
  audioLoadService?: AudioLoadService;
  audioEventSink?: (events: MusicalEvent[], masterVolume: number) => Promise<void>;
  runtimeDispatch?: (message: RuntimeHostMessage) => Promise<RuntimeRunnerMessage[]>;
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  autoSaveCooldownMs?: number;
};

const LOOKAHEAD_MS = 20;
const MAX_CATCHUP_MS = 250;
const DEFAULT_AUTO_SAVE_COOLDOWN_MS = 2000;
const TAURI_DISPLAY_DRAIN_MS = 66;
const ASYNC_RUNTIME_SUPPRESS_MS = 120;
const PPQN = 24;

type ScheduledEvents = { dueMs: number; events: MusicalEvent[] };
type ScheduledMidi = { dueMs: number; bytes: Uint8Array };

export function createSimulatorRuntime(scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8), deps: RuntimeDeps = {}): SimulatorRuntime {
  const nativeRuntimeMode = deps.runtimeDispatch
    ? true
    : typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
  const runner = deps.runner ?? null;
  if (!nativeRuntimeMode && !runner) {
    throw new Error("Desktop runtime requires Tauri native runtime or an injected test runner");
  }
  let coreState = () => runner?.getState() ?? null;
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
  let prevPlaying = latestFrame.transport.playing;
  let prevStopLatched = latestFrame.settings?.stopLatched ?? false;
  let prevPpqnPulse = latestFrame.transport.ppqnPulse;
  let lastSyncedPlaybackConfig = "";
  let internalPulseRemainder = 0;
  const eventQueue: ScheduledEvents[] = [];
  const midiQueue: ScheduledMidi[] = [];
  const listeners = new Set<RuntimeListener>();
  const eventListeners = new Set<EventsListener>();
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };
  let audioError: string | null = null;
  const autoSaveCooldownMs = deps.autoSaveCooldownMs ?? DEFAULT_AUTO_SAVE_COOLDOWN_MS;
  let pendingDefaultSave: ConfigPayload | null = null;
  let pendingDefaultSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let runtimeUpdateEpoch = 0;
  let lastAsyncRuntimeSeq = 0;
  let lastTauriDrainAt = 0;
  let tauriDrainInFlight = false;
  let ignoreAsyncUntilMs = 0;

  const store = deps.store ?? createLocalStorageConfigStore();
  const tauriMidi: RuntimeMidiService = deps.midiService ?? new TauriMidiService();
  const audioLoadService: AudioLoadService = deps.audioLoadService ?? new TauriAudioLoadService();
  const audioEventSink = deps.audioEventSink ?? sendEventsToAudio;
  const runtimeDispatch = deps.runtimeDispatch ?? (nativeRuntimeMode ? (message: RuntimeHostMessage) => tauriCoreRunner.dispatchRuntime(message) : null);
  const invokeBridge = deps.invoke ?? invoke;
  const runtimeMessagesReady = nativeRuntimeMode && !deps.runtimeDispatch
    ? tauriCoreRunner.listenRuntimeMessages((batch) => {
        applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now());
      }).catch((err) => {
        console.error("[Runtime] listenRuntimeMessages failed:", err);
        audioError = `runtime messages failed: ${err instanceof Error ? err.message : String(err)}`;
        publishSnapshot();
      })
    : Promise.resolve<() => void>(() => {});
  void runtimeMessagesReady;

  const frameSettings = () => latestFrame.settings;

  let selectedOutId: string | null = null;
  let selectedInId: string | null = null;
  const midi = {
    async listOutputs() {
      return await tauriMidi.listOutputs();
    },
    async listInputs() {
      return await tauriMidi.listInputs();
    },
    async selectOutput(id: string | null) {
      selectedOutId = id;
      return await tauriMidi.selectOutput(id);
    },
    async selectInput(id: string | null) {
      selectedInId = id;
      return await tauriMidi.selectInput(id);
    },
    async send(bytes: Uint8Array) {
      await tauriMidi.send(bytes);
    },
    getSelectedOutputId() {
      return selectedOutId;
    },
    getSelectedInputId() {
      return selectedInId;
    }
  };

  let extPulses = 0;
  const extMsgs: Array<"start" | "continue" | "stop"> = [];

  function flushPendingDefaultSave() {
    if (pendingDefaultSaveTimer !== null) {
      clearTimeout(pendingDefaultSaveTimer);
      pendingDefaultSaveTimer = null;
    }
    if (!pendingDefaultSave) return;
    const payload = pendingDefaultSave;
    pendingDefaultSave = null;
    if (nativeRuntimeMode) {
      return;
    }
    store.saveDefault(payload);
    applyRuntimeResult({ type: "save_default_result", ok: true, isAuto: true }, performance.now(), performance.now());
    publishSnapshot();
  }

  function cancelPendingDefaultSave() {
    if (pendingDefaultSaveTimer !== null) {
      clearTimeout(pendingDefaultSaveTimer);
      pendingDefaultSaveTimer = null;
    }
    pendingDefaultSave = null;
  }

  function scheduleDefaultSave(payload: ConfigPayload) {
    pendingDefaultSave = payload;
    if (pendingDefaultSaveTimer !== null) clearTimeout(pendingDefaultSaveTimer);
    pendingDefaultSaveTimer = setTimeout(() => {
      pendingDefaultSaveTimer = null;
      flushPendingDefaultSave();
      publishSnapshot();
    }, autoSaveCooldownMs);
  }

  function snapshotFromCore(frame: RuntimeSnapshot): SimulatorSnapshot {
    const state = coreState() as any;
    const settings = frame.settings;
    const flash = settings?.transportFlash ?? state?.system?.transportFlash ?? "none";
    const combined = settings?.combinedModifierHeld ?? state?.system?.combinedModifierHeld ?? false;
    return {
      frame,
      neoKeyLeds: {
        back: "solid_red",
        space: !frame.transport.playing ? "off" : flash === "measure" ? "measure" : flash === "beat" ? "beat" : "off",
        shift: combined ? "solid_blue" : shiftActive ? "solid_yellow" : "off",
        fn: combined ? "solid_blue" : (settings?.fnHeld ?? state?.system?.fnHeld ?? false) ? "solid_yellow" : "off"
      },
      displayBrightness: settings?.displayBrightness ?? state?.runtimeConfig?.displayBrightness ?? 75,
      buttonBrightness: settings?.buttonBrightness ?? state?.runtimeConfig?.buttonBrightness ?? 75,
      masterVolume: settings?.masterVolume ?? state?.runtimeConfig?.masterVolume ?? 100,
      voiceStealingMode: (settings?.voiceStealingMode ?? state?.runtimeConfig?.sound?.voiceStealingMode ?? "balanced") as any,
      audioLoad,
      audioError,
      instruments: settings?.instruments ?? (Array.isArray(state?.runtimeConfig?.instruments) ? (state.runtimeConfig.instruments as unknown[]) : []),
      mixer: settings?.mixer ?? state?.runtimeConfig?.mixer ?? { buses: [] },
      panPositions: settings?.panPositions ?? state?.runtimeConfig?.panPositions ?? PAN_POSITION_COUNT,
      autoSaveFlash: settings?.autoSaveFlash ?? state?.system?.autoSaveFlash ?? "none"
    };
  }

  function publishSnapshot() {
    const snapshot = snapshotFromCore(latestFrame);
    for (const listener of listeners) listener(snapshot);
  }

  function publishEvents(events: MusicalEvent[]) {
    if (events.length === 0) return;
    if (!nativeRuntimeMode) sendMidiForEvents(events, performance.now());
    if (!nativeRuntimeMode) void audioEventSink(events, snapshotFromCore(latestFrame).masterVolume);
    for (const listener of eventListeners) listener(events);
  }

  function shouldBypassLocalRunner(input: DeviceInput) {
    void input;
    return nativeRuntimeMode;
  }

  function syncPlaybackConfigIfNeeded() {
    if (!nativeRuntimeMode) return;
    const settings = frameSettings();
    if (!settings) return;
    const midi = settings.midi;
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

  function mirrorRuntimeMessage(message: RuntimeHostMessage) {
    if (!runtimeDispatch) return;
    runtimeUpdateEpoch += 1;
    ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
    void runtimeDispatch(message)
      .then((messages) => {
        processRunnerMessages(messages, performance.now(), performance.now());
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
    processRunnerMessages(messages, nowMs, nowMs);
    publishSnapshot();
  }

  function maybeDrainTauriRuntimeMessages(nowMs: number) {
    if (!nativeRuntimeMode || tauriDrainInFlight) return;
    if (nowMs - lastTauriDrainAt < TAURI_DISPLAY_DRAIN_MS) return;
    tauriDrainInFlight = true;
    lastTauriDrainAt = nowMs;
    void tauriCoreRunner.drainRuntimeMessages()
      .then((batches) => {
        for (const batch of batches) applyAsyncRuntimeBatch(batch.seq, batch.messages, nowMs);
      })
      .catch((err) => {
        console.error("[Runtime] drainRuntimeMessages failed:", err);
      })
      .finally(() => {
        tauriDrainInFlight = false;
      });
  }

  function enqueueEvents(events: MusicalEvent[], dueMs: number) {
    if (events.length === 0) return;
    eventQueue.push({ dueMs, events });
  }

  function flushDueEvents(nowMs: number) {
    if (eventQueue.length === 0) return;
    eventQueue.sort((a, b) => a.dueMs - b.dueMs);
    while (eventQueue.length > 0 && eventQueue[0].dueMs <= nowMs) {
      const due = eventQueue.shift();
      if (!due) break;
      publishEvents(due.events);
    }
  }

  function flushDueMidi(nowMs: number) {
    if (midiQueue.length === 0) return;
    midiQueue.sort((a, b) => a.dueMs - b.dueMs);
    while (midiQueue.length > 0 && midiQueue[0].dueMs <= nowMs) {
      const due = midiQueue.shift();
      if (!due) break;
      void midi.send(due.bytes);
    }
  }

  function scheduleMidi(bytes: Uint8Array, dueMs: number) {
    midiQueue.push({ bytes, dueMs });
  }

  function sendMidiForEvents(events: MusicalEvent[], nowMs: number) {
    const state = coreState() as any;
    const cfg = state.runtimeConfig.midi;
    if (!cfg.enabled || !cfg.outId) return;
    if (!state.transport.playing) return;
    const instruments: any[] = Array.isArray(state.runtimeConfig.instruments) ? state.runtimeConfig.instruments : [];
    for (const event of events) {
      const slot = Math.max(0, Math.min(15, (event as any).channel | 0));
      const inst = instruments[slot];
      if (inst?.midi?.enabled !== true) continue;
      const channel = Math.max(0, Math.min(15, (inst?.midi?.channel ?? slot) | 0));
      if (event.type === "note_on") {
        const note = Math.max(0, Math.min(127, event.note | 0));
        const vel = Math.max(1, Math.min(127, event.velocity | 0));
        scheduleMidi(new Uint8Array([0x90 | channel, note, vel]), nowMs);
        if (typeof event.durationMs === "number") {
          scheduleMidi(new Uint8Array([0x80 | channel, note, 0]), nowMs + Math.max(1, Math.min(10_000, event.durationMs)));
        }
        continue;
      }
      if (event.type === "note_off") {
        scheduleMidi(new Uint8Array([0x80 | channel, Math.max(0, Math.min(127, event.note | 0)), 0]), nowMs);
        continue;
      }
      scheduleMidi(new Uint8Array([0xb0 | channel, Math.max(0, Math.min(127, event.controller | 0)), Math.max(0, Math.min(127, event.value | 0))]), nowMs);
    }
  }

  function sendMidiTransportIfNeeded(nowMs: number) {
    if (nativeRuntimeMode) {
      prevPlaying = latestFrame.transport.playing;
      prevStopLatched = latestFrame.settings?.stopLatched ?? prevStopLatched;
      prevPpqnPulse = latestFrame.transport.ppqnPulse;
      return;
    }
    const state = coreState() as any;
    const cfg = state.runtimeConfig.midi;
    if (!cfg.enabled || cfg.syncMode !== "internal" || !cfg.outId) {
      prevPlaying = state.transport.playing;
      prevStopLatched = state.system.stopLatched;
      prevPpqnPulse = state.transport.ppqnPulse;
      return;
    }
    if (prevPlaying !== state.transport.playing) {
      if (!prevPlaying && state.transport.playing) scheduleMidi(new Uint8Array([prevStopLatched ? 0xfa : 0xfb]), nowMs);
      else if (prevPlaying && !state.transport.playing) scheduleMidi(new Uint8Array([0xfc]), nowMs);
    }
    if (cfg.clockOutEnabled && state.transport.playing) {
      for (let pulse = prevPpqnPulse + 1; pulse <= state.transport.ppqnPulse; pulse += 1) {
        scheduleMidi(new Uint8Array([0xf8]), nowMs);
      }
    }
    prevPlaying = state.transport.playing;
    prevStopLatched = state.system.stopLatched;
    prevPpqnPulse = state.transport.ppqnPulse;
  }

  function processRunnerMessages(messages: RuntimeRunnerMessage[], dueMs: number, nowMs: number) {
    let snapshotSeen = false;
    for (const message of messages) {
      if (message.type === "snapshot") {
        const snapshot = message.snapshot;
        if (snapshot.oled && !(snapshot.oled.pixels instanceof Uint8Array)) {
          snapshot.oled = { ...snapshot.oled, pixels: new Uint8Array(Object.values(snapshot.oled.pixels as any)) };
        }
        latestFrame = snapshot;
        snapshotSeen = true;
        continue;
      }
      if (message.type === "musical_events") {
        if (nativeRuntimeMode) continue;
        enqueueEvents(message.events, dueMs);
        continue;
      }
      if (message.type === "platform_effects") {
        if (nativeRuntimeMode) continue;
        applyEffects(message.effects.filter((effect) => effect.type !== "audio_command"), dueMs, nowMs);
        continue;
      }
      if (message.type === "audio_commands") {
        if (nativeRuntimeMode) continue;
        for (const command of message.commands) execAudioCommand(command);
        continue;
      }
      if ((message as any).type === "audio_error") {
        audioError = (message as any).error ?? null;
        if (audioError) console.warn("[Runtime] audio error:", audioError);
      }
    }
    if (!snapshotSeen && !nativeRuntimeMode && runner) latestFrame = runner.getFrame();
  }

  function applyRuntimeResult(result: RuntimeStoreResult, dueMs: number, nowMs: number) {
    if (runtimeDispatch) {
      runtimeUpdateEpoch += 1;
      ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
      void runtimeDispatch({ type: "runtime_result", result })
        .then((messages) => {
          processRunnerMessages(messages, dueMs, nowMs);
          publishSnapshot();
        })
        .catch((err) => {
          console.error("[Runtime] runtimeDispatch (result) failed:", err);
          audioError = `dispatch failed: ${err instanceof Error ? err.message : String(err)}`;
          publishSnapshot();
        });
      return;
    }

    if (!runner) return;
    processRunnerMessages(runner.dispatch({ type: "runtime_result", result }), dueMs, nowMs);
  }

  function execAudioCommand(command: RuntimeAudioCommand) {
    void invokeBridge("audio_command", { command }).catch((err) => {
      console.error("[Runtime] audio command failed:", err);
      if (command.type !== "sample_preview") return;
      applyRuntimeResult({ type: "sample_preview_error", message: err instanceof Error ? err.message : String(err) }, performance.now(), performance.now());
      publishSnapshot();
    });
  }

  function execEffect(effect: RuntimePlatformEffect): RuntimeStoreResult | null {
    try {
      if (effect.type === "store_list_presets") return { type: "list_presets_result", names: store.listPresets() };
      if (effect.type === "store_load_preset") {
        cancelPendingDefaultSave();
        return { type: "load_preset_result", name: effect.name, payload: store.loadPreset(effect.name) };
      }
      if (effect.type === "store_save_preset") return { type: "save_preset_result", name: effect.name, outcome: store.savePreset(effect.name, effect.payload as ConfigPayload) };
      if (effect.type === "store_delete_preset") return { type: "delete_preset_result", name: effect.name, ok: store.deletePreset(effect.name) };
      if (effect.type === "store_load_default") {
        cancelPendingDefaultSave();
        return { type: "load_default_result", payload: store.loadDefault() };
      }
      if (effect.type === "store_save_default") {
        if (effect.mode === "deferred") {
          scheduleDefaultSave(effect.payload as ConfigPayload);
          return null;
        }
        cancelPendingDefaultSave();
        store.saveDefault(effect.payload as ConfigPayload);
        return { type: "save_default_result", ok: true };
      }
      if (effect.type === "midi_list_outputs_request") {
        void midi.listOutputs().then((outputs) => {
          applyRuntimeResult({ type: "midi_list_outputs_result", outputs }, performance.now(), performance.now());
          publishSnapshot();
        });
        return { type: "midi_list_outputs_result", outputs: [] };
      }
      if (effect.type === "midi_list_inputs_request") {
        void midi.listInputs().then((inputs) => {
          applyRuntimeResult({ type: "midi_list_inputs_result", inputs }, performance.now(), performance.now());
          publishSnapshot();
        });
        return { type: "midi_list_inputs_result", inputs: [] };
      }
      if (effect.type === "midi_select_output") {
        void midi.selectOutput(effect.id).then((res) => {
          applyRuntimeResult({ type: "midi_status", ok: res.ok, message: res.message, selectedOutId: effect.id }, performance.now(), performance.now());
          publishSnapshot();
        });
        return { type: "midi_status", ok: true };
      }
      if (effect.type === "midi_select_input") {
        void midi.selectInput(effect.id).then((res) => {
          applyRuntimeResult({ type: "midi_status", ok: res.ok, message: res.message, selectedInId: effect.id }, performance.now(), performance.now());
          publishSnapshot();
        });
        return { type: "midi_status", ok: true };
      }
      if (effect.type === "midi_panic") {
        const now = performance.now();
        scheduleMidi(new Uint8Array([0xfc]), now);
        for (let channel = 0; channel < 16; channel += 1) {
          scheduleMidi(new Uint8Array([0xb0 | channel, 120, 0]), now);
          scheduleMidi(new Uint8Array([0xb0 | channel, 123, 0]), now);
        }
        midiQueue.length = 0;
        return { type: "midi_status", ok: true, message: "Panic sent" };
      }
      if (effect.type === "sample_list_request") {
        void invokeBridge("sample_list", { dir: effect.dir }).then((entries) => {
          const safe = Array.isArray(entries)
            ? entries.map((entry: any) => ({ name: String(entry?.name ?? ""), path: String(entry?.path ?? ""), isDir: Boolean(entry?.isDir ?? entry?.is_dir ?? false) })).filter((entry) => entry.name.length > 0)
            : [];
          applyRuntimeResult({ type: "sample_list_result", instrumentSlot: effect.instrumentSlot, sampleSlot: effect.sampleSlot, dir: effect.dir, entries: safe }, performance.now(), performance.now());
          publishSnapshot();
        }).catch((err) => {
          applyRuntimeResult({ type: "sample_list_error", instrumentSlot: effect.instrumentSlot, sampleSlot: effect.sampleSlot, dir: effect.dir, message: err instanceof Error ? err.message : String(err) }, performance.now(), performance.now());
          publishSnapshot();
        });
        return { type: "sample_list_result", instrumentSlot: effect.instrumentSlot, sampleSlot: effect.sampleSlot, dir: effect.dir, entries: [] };
      }
      return null;
    } catch (err) {
      return { type: "store_error", message: err instanceof Error ? err.message : "Store error" };
    }
  }

  function applyEffects(initial: RuntimePlatformEffect[], dueMs: number, nowMs: number) {
    const queue = initial.slice();
    while (queue.length > 0) {
      const effect = queue.shift();
      if (!effect) break;
      const result = execEffect(effect);
      if (!result) continue;
      applyRuntimeResult(result, dueMs, nowMs);
    }
  }

  function dispatchToRunner(input: DeviceInput) {
    const now = performance.now();
    syncPlaybackConfigIfNeeded();
    if (shouldBypassLocalRunner(input)) {
      mirrorRuntimeMessage({ type: "device_input", input });
      return;
    }
    mirrorRuntimeMessage({ type: "device_input", input });
    if (!runner) return;
    processRunnerMessages(runner.dispatch({ type: "device_input", input }), now, now);
    sendMidiTransportIfNeeded(now);
    flushDueEvents(now);
    flushDueMidi(now);
    publishSnapshot();
  }

  if (!nativeRuntimeMode) {
    const defaultPayload = store.loadDefault();
    if (defaultPayload) {
      applyRuntimeResult({ type: "load_default_result", payload: defaultPayload }, performance.now(), performance.now());
    }
  }

  syncPlaybackConfigIfNeeded();

  void tauriMidi.listenMidiIn((data: Uint8Array) => {
    if (nativeRuntimeMode) {
      void tauriCoreRunner
        .handleMidiRealtime(data)
        .then((messages) => {
          processRunnerMessages(messages, performance.now(), performance.now());
          publishSnapshot();
        })
        .catch((err) => { console.error("[Runtime] handleMidiRealtime failed:", err); });
      return;
    }

    for (const byte of data) {
      if (byte === 0xf8) extPulses += 1;
      else if (byte === 0xfa) extMsgs.push("start");
      else if (byte === 0xfb) extMsgs.push("continue");
      else if (byte === 0xfc) extMsgs.push("stop");
    }
  });

  void audioLoadService.listenAudioLoad((status) => {
    audioLoad = { ratio: Math.max(0, Math.min(2, status.ratio)), voiceSteal: status.voiceSteal };
    publishSnapshot();
  });

  if (!nativeRuntimeMode) {
    void midi.listOutputs().then((outputs) => {
      applyRuntimeResult({ type: "midi_list_outputs_result", outputs }, performance.now(), performance.now());
      publishSnapshot();
    });
    void midi.listInputs().then((inputs) => {
      applyRuntimeResult({ type: "midi_list_inputs_result", inputs }, performance.now(), performance.now());
      publishSnapshot();
    });
  }

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
      if (nativeRuntimeMode && runtimeDispatch) {
        runtimeUpdateEpoch += 1;
        ignoreAsyncUntilMs = performance.now() + ASYNC_RUNTIME_SUPPRESS_MS;
        void runtimeDispatch({ type: "transport_pulse_step", pulses: 0, source: "internal" })
          .then((messages) => {
            processRunnerMessages(messages, performance.now(), performance.now());
            publishSnapshot();
          })
          .catch((err) => {
            console.error("[Runtime] initial pulse_step failed:", err);
            audioError = `initial pulse_step failed: ${err instanceof Error ? err.message : String(err)}`;
            if (latestFrame.transport.playing) {
              latestFrame = { ...latestFrame, transport: { ...latestFrame.transport, playing: false } };
            }
            publishSnapshot();
          });
      }

      scheduler.start((nowMs, elapsedMs) => {
        syncPlaybackConfigIfNeeded();
        const cfg = frameSettings()?.midi ?? ((coreState() as any).runtimeConfig.midi as any);
        if (!nativeRuntimeMode && cfg.enabled) {
          if (cfg.outId !== midi.getSelectedOutputId()) void midi.selectOutput(cfg.outId);
          if (cfg.inId !== midi.getSelectedInputId()) void midi.selectInput(cfg.inId);
        } else if (!nativeRuntimeMode) {
          if (midi.getSelectedOutputId() !== null) void midi.selectOutput(null);
          if (midi.getSelectedInputId() !== null) void midi.selectInput(null);
        }

        const safeElapsedMs = Math.min(elapsedMs, MAX_CATCHUP_MS);
        const externalClock = cfg.syncMode === "external" && cfg.clockInEnabled;

        if (nativeRuntimeMode) {
          void safeElapsedMs;
          void externalClock;
          maybeDrainTauriRuntimeMessages(nowMs);
          sendMidiTransportIfNeeded(nowMs);
          flushDueEvents(nowMs);
          flushDueMidi(nowMs);
          publishSnapshot();
          return;
        }

        if (externalClock) {
          internalPulseRemainder = 0;
          while (extMsgs.length > 0) {
            const message = extMsgs.shift();
            if (!message) break;
            mirrorRuntimeMessage({ type: "midi_realtime", message });
            if (!runner) continue;
            processRunnerMessages(runner.dispatch({ type: "midi_realtime", message }), nowMs + LOOKAHEAD_MS, nowMs);
          }
          if (extPulses > 0) {
            const pulses = extPulses;
            extPulses = 0;
            mirrorRuntimeMessage({ type: "midi_realtime", message: "clock", pulses });
            if (!runner) {
              sendMidiTransportIfNeeded(nowMs);
              flushDueEvents(nowMs);
              flushDueMidi(nowMs);
              publishSnapshot();
              return;
            }
            processRunnerMessages(runner.dispatch({ type: "midi_realtime", message: "clock", pulses }), nowMs + LOOKAHEAD_MS, nowMs);
          }
          mirrorRuntimeMessage({ type: "transport_pulse_step", pulses: 0, source: "external" });
          if (!runner) {
            sendMidiTransportIfNeeded(nowMs);
            flushDueEvents(nowMs);
            flushDueMidi(nowMs);
            publishSnapshot();
            return;
          }
          processRunnerMessages(runner.dispatch({ type: "transport_pulse_step", pulses: 0, source: "external" }), nowMs + LOOKAHEAD_MS, nowMs);
        } else {
          let pulses = 0;
          if (latestFrame.transport.playing) {
            internalPulseRemainder += ((latestFrame.transport.bpm / 60) * PPQN * safeElapsedMs) / 1000;
            pulses = Math.floor(internalPulseRemainder);
            internalPulseRemainder -= pulses;
          } else {
            internalPulseRemainder = 0;
          }
          mirrorRuntimeMessage({ type: "transport_pulse_step", pulses, source: "internal" });
          if (!runner) {
            sendMidiTransportIfNeeded(nowMs);
            flushDueEvents(nowMs);
            flushDueMidi(nowMs);
            publishSnapshot();
            return;
          }
          processRunnerMessages(runner.dispatch({ type: "transport_pulse_step", pulses, source: "internal" }), nowMs + LOOKAHEAD_MS, nowMs);
        }

        sendMidiTransportIfNeeded(nowMs);
        flushDueEvents(nowMs);
        flushDueMidi(nowMs);
        publishSnapshot();
      });
      publishSnapshot();
    },
    stop() {
      flushPendingDefaultSave();
      publishSnapshot();
      scheduler.stop();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(snapshotFromCore(latestFrame));
      return () => listeners.delete(listener);
    },
    subscribeEvents(listener) {
      eventListeners.add(listener);
      return () => eventListeners.delete(listener);
    },
    getSnapshot() {
      return snapshotFromCore(latestFrame);
    }
  };
}
