import { getBehavior } from "@cellsymphony/behavior-api";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import {
  applyConfigPayload,
  applyStoreResult,
  createInitialState,
  emergencyBrake,
  routeInput,
  tick,
  toSimulatorFrame,
  type PlatformEffect,
  type PlatformState,
  type StoreResult
} from "@cellsymphony/platform-core";
import { createIntervalRuntimeScheduler, type RuntimeScheduler } from "./runtimeScheduler";
import type { EventsListener, InputAction, RuntimeListener, SimulatorSnapshot } from "./types";
import { createLocalStorageConfigStore } from "./configStore";
import { TauriMidiService } from "./midi/tauriMidi";
import { invoke } from "@tauri-apps/api/core";
import type { ConfigPayload } from "@cellsymphony/platform-core";
import { TauriAudioLoadService, type AudioLoadService, type AudioLoadStatus } from "../audio/audioLoadEvents";

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

type RuntimeDeps = {
  store?: RuntimeStore;
  midiService?: RuntimeMidiService;
  audioLoadService?: AudioLoadService;
  invoke?: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  autoSaveCooldownMs?: number;
};

const LOOKAHEAD_MS = 20;
const MAX_CATCHUP_MS = 250;
const DEFAULT_AUTO_SAVE_COOLDOWN_MS = 2000;

type ScheduledEvents = {
  dueMs: number;
  events: MusicalEvent[];
};

type ScheduledMidi = {
  dueMs: number;
  bytes: Uint8Array;
};

export function createSimulatorRuntime(scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8), deps: RuntimeDeps = {}): SimulatorRuntime {
  function activeBehavior(): BehaviorEngine<any, any> {
    return getBehavior((state.runtimeConfig as any).activeBehavior) ?? getBehavior("sequencer")!;
  }
  let state: PlatformState<any> = createInitialState(getBehavior("life")!);
  let shiftActive = false;
  let prevPlaying = state.transport.playing;
  let prevStopLatched = state.system.stopLatched;
  let prevPpqnPulse = state.transport.ppqnPulse;
  const eventQueue: ScheduledEvents[] = [];
  const midiQueue: ScheduledMidi[] = [];
  const listeners = new Set<RuntimeListener>();
  const eventListeners = new Set<EventsListener>();
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };
  const autoSaveCooldownMs = deps.autoSaveCooldownMs ?? DEFAULT_AUTO_SAVE_COOLDOWN_MS;
  let pendingDefaultSave: ConfigPayload | null = null;
  let pendingDefaultSaveTimer: ReturnType<typeof setTimeout> | null = null;

  function flushPendingDefaultSave() {
    if (pendingDefaultSaveTimer !== null) {
      clearTimeout(pendingDefaultSaveTimer);
      pendingDefaultSaveTimer = null;
    }
    if (!pendingDefaultSave) return;
    store.saveDefault(pendingDefaultSave);
    pendingDefaultSave = null;
    state = applyStoreResult(state, { type: "save_default_result", ok: true, isAuto: true }, activeBehavior()).state;
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

  function snapshotFromState(next: typeof state): SimulatorSnapshot {
    const frame = toSimulatorFrame(next, activeBehavior(), { audioLoad });
    const flash = next.system.transportFlash; // read from core state, same as OLED
    return {
      frame,
      neoKeyLeds: {
        back: "solid_red",
        space: !frame.transport.playing ? "off" : flash === "measure" ? "measure" : flash === "beat" ? "beat" : "off",
        shift: shiftActive ? "solid_yellow" : "off",
        fn: "off"
      },
      displayBrightness: (next as any).runtimeConfig.displayBrightness ?? 75,
      buttonBrightness: (next as any).runtimeConfig.buttonBrightness ?? 75,
      masterVolume: (next as any).runtimeConfig.masterVolume ?? 100,
      voiceStealingMode: ((next as any).runtimeConfig.sound?.voiceStealingMode ?? "balanced") as any,
      audioLoad,
      instruments: Array.isArray((next as any).runtimeConfig.instruments) ? ((next as any).runtimeConfig.instruments as unknown[]) : [],
      mixer: (next as any).runtimeConfig.mixer ?? { buses: [] },
      autoSaveFlash: (next.system as any).autoSaveFlash ?? "none"
    };
  }

  function publishSnapshot() {
    const snapshot = snapshotFromState(state);
    for (const listener of listeners) {
      listener(snapshot);
    }
  }

  function publishEvents(events: MusicalEvent[]) {
    if (events.length === 0) return;
    sendMidiForEvents(events, performance.now());
    for (const listener of eventListeners) {
      listener(events);
    }
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

  function midiStopOnly(nowMs: number) {
    const cfg = state.runtimeConfig.midi;
    if (!cfg.enabled || cfg.syncMode !== "internal") return;
    if (!cfg.outId) return;
    scheduleMidi(new Uint8Array([0xfc]), nowMs);
    midiQueue.length = 0; // drop pending note-offs
  }

  function sendMidiForEvents(events: MusicalEvent[], nowMs: number) {
    const cfg = state.runtimeConfig.midi;
    if (!cfg.enabled || !cfg.outId) return;
    if (!state.transport.playing) return;
    const instruments: any[] = Array.isArray((state.runtimeConfig as any).instruments)
      ? ((state.runtimeConfig as any).instruments as any[])
      : [];
    for (const e of events) {
      const slot = Math.max(0, Math.min(15, (e as any).channel | 0));
      const inst = instruments[slot];
      const instMidiEnabled = inst?.midi?.enabled === true;
      if (!instMidiEnabled) continue;
      const instMidiChannel = Math.max(0, Math.min(15, (inst?.midi?.channel ?? slot) | 0));
      if (e.type === "note_on") {
        const ch = instMidiChannel;
        const note = Math.max(0, Math.min(127, e.note | 0));
        const vel = Math.max(1, Math.min(127, e.velocity | 0));
        scheduleMidi(new Uint8Array([0x90 | ch, note, vel]), nowMs);
        if (typeof e.durationMs === "number") {
          const len = Math.max(1, Math.min(10_000, e.durationMs));
          scheduleMidi(new Uint8Array([0x80 | ch, note, 0]), nowMs + len);
        }
        continue;
      }
      if (e.type === "note_off") {
        const ch = instMidiChannel;
        const note = Math.max(0, Math.min(127, e.note | 0));
        scheduleMidi(new Uint8Array([0x80 | ch, note, 0]), nowMs);
        continue;
      }
      if (e.type === "cc") {
        const ch = instMidiChannel;
        const cc = Math.max(0, Math.min(127, e.controller | 0));
        const val = Math.max(0, Math.min(127, e.value | 0));
        scheduleMidi(new Uint8Array([0xb0 | ch, cc, val]), nowMs);
      }
    }
  }

  function sendMidiTransportIfNeeded(nowMs: number) {
    const cfg = state.runtimeConfig.midi;
    if (!cfg.enabled || cfg.syncMode !== "internal" || !cfg.outId) {
      prevPlaying = state.transport.playing;
      prevStopLatched = state.system.stopLatched;
      prevPpqnPulse = state.transport.ppqnPulse;
      return;
    }

    if (prevPlaying !== state.transport.playing) {
      if (!prevPlaying && state.transport.playing) {
        // stop->play uses Start, pause->play uses Continue
        const b = prevStopLatched ? 0xfa : 0xfb;
        scheduleMidi(new Uint8Array([b]), nowMs);
      } else if (prevPlaying && !state.transport.playing) {
        scheduleMidi(new Uint8Array([0xfc]), nowMs);
      }
    }

    if (cfg.clockOutEnabled && state.transport.playing) {
      const from = prevPpqnPulse;
      const to = state.transport.ppqnPulse;
      for (let p = from + 1; p <= to; p += 1) {
        scheduleMidi(new Uint8Array([0xf8]), nowMs);
      }
    }

    prevPlaying = state.transport.playing;
    prevStopLatched = state.system.stopLatched;
    prevPpqnPulse = state.transport.ppqnPulse;
  }

  function applyInput(input: DeviceInput) {
    if (input.type === "button_s" && shiftActive) {
      if (state.runtimeConfig.midi.syncMode === "external") {
        // In external sync mode, Shift+S is reserved for resync (handled in core).
      } else {
        const result = emergencyBrake(state);
        state = result.state;
        midiStopOnly(performance.now());
        prevPlaying = state.transport.playing;
        prevStopLatched = state.system.stopLatched;
        prevPpqnPulse = state.transport.ppqnPulse;
        publishEvents(result.events);
        publishSnapshot();
        return;
      }
    }
    const result = routeInput(state, input, activeBehavior());
    state = result.state;
    enqueueEvents(result.events, performance.now());
    applyEffects(result.effects);
    sendMidiTransportIfNeeded(performance.now());
    flushDueEvents(performance.now());
    flushDueMidi(performance.now());
    publishSnapshot();
  }

  const store = deps.store ?? createLocalStorageConfigStore();
  {
    const payload = store.loadDefault();
    if (payload) {
      const loaded = applyConfigPayload(state, payload, activeBehavior());
      state = { ...loaded, system: { ...loaded.system, currentPresetName: null } };
    }
  }
  const tauriMidi: RuntimeMidiService = deps.midiService ?? new TauriMidiService();
  const audioLoadService: AudioLoadService = deps.audioLoadService ?? new TauriAudioLoadService();
  const invokeBridge = deps.invoke ?? invoke;

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
  const extMsgs: ("start" | "continue" | "stop")[] = [];

  void tauriMidi.listenMidiIn((data: Uint8Array) => {
    for (const b of data) {
      if (b === 0xf8) {
        extPulses += 1;
      } else if (b === 0xfa) {
        extMsgs.push("start");
      } else if (b === 0xfb) {
        extMsgs.push("continue");
      } else if (b === 0xfc) {
        extMsgs.push("stop");
      }
    }
  });

  void audioLoadService.listenAudioLoad((status) => {
    audioLoad = { ratio: Math.max(0, Math.min(2, status.ratio)), voiceSteal: status.voiceSteal };
    publishSnapshot();
  });

  // Prime MIDI port lists on boot.
  void midi.listOutputs().then((outputs) => {
    const applied = applyStoreResult(state, { type: "midi_list_outputs_result", outputs } as any, activeBehavior());
    state = applied.state;
    publishSnapshot();
  });
  void midi.listInputs().then((inputs) => {
    const applied = applyStoreResult(state, { type: "midi_list_inputs_result", inputs } as any, activeBehavior());
    state = applied.state;
    publishSnapshot();
  });



  function execEffect(effect: PlatformEffect): StoreResult | null {
    try {
      if (effect.type === "store_list_presets") {
        return { type: "list_presets_result", names: store.listPresets() };
      }
      if (effect.type === "store_load_preset") {
        cancelPendingDefaultSave();
        return { type: "load_preset_result", name: effect.name, payload: store.loadPreset(effect.name) };
      }
      if (effect.type === "store_save_preset") {
        const outcome = store.savePreset(effect.name, effect.payload);
        return { type: "save_preset_result", name: effect.name, outcome };
      }
      if (effect.type === "store_delete_preset") {
        const ok = store.deletePreset(effect.name);
        return { type: "delete_preset_result", name: effect.name, ok };
      }
      if (effect.type === "store_load_default") {
        cancelPendingDefaultSave();
        return { type: "load_default_result", payload: store.loadDefault() };
      }
      if (effect.type === "store_save_default") {
        if (effect.mode === "deferred") {
          scheduleDefaultSave(effect.payload);
          return null;
        }
        cancelPendingDefaultSave();
        store.saveDefault(effect.payload);
        return { type: "save_default_result", ok: true };
      }
      if (effect.type === "midi_list_outputs_request") {
        // Fire-and-forget async, return empty immediately.
        void midi.listOutputs().then((outputs) => {
          const applied = applyStoreResult(state, { type: "midi_list_outputs_result", outputs } as any, activeBehavior());
          state = applied.state;
          publishSnapshot();
        });
        return { type: "midi_list_outputs_result", outputs: [] } as any;
      }
      if (effect.type === "midi_list_inputs_request") {
        void midi.listInputs().then((inputs) => {
          const applied = applyStoreResult(state, { type: "midi_list_inputs_result", inputs } as any, activeBehavior());
          state = applied.state;
          publishSnapshot();
        });
        return { type: "midi_list_inputs_result", inputs: [] } as any;
      }
      if (effect.type === "midi_select_output") {
        void midi.selectOutput(effect.id).then((res) => {
          const applied = applyStoreResult(state, { type: "midi_status", ok: res.ok, message: res.message, selectedOutId: effect.id } as any, activeBehavior());
          state = applied.state;
          publishSnapshot();
        });
        return { type: "midi_status", ok: true } as any;
      }
      if (effect.type === "midi_select_input") {
        void midi.selectInput(effect.id).then((res) => {
          const applied = applyStoreResult(state, { type: "midi_status", ok: res.ok, message: res.message, selectedInId: effect.id } as any, activeBehavior());
          state = applied.state;
          publishSnapshot();
        });
        return { type: "midi_status", ok: true } as any;
      }
      if (effect.type === "midi_panic") {
        const now = performance.now();
        scheduleMidi(new Uint8Array([0xfc]), now);
        for (let ch = 0; ch < 16; ch += 1) {
          scheduleMidi(new Uint8Array([0xb0 | ch, 120, 0]), now);
          scheduleMidi(new Uint8Array([0xb0 | ch, 123, 0]), now);
        }
        midiQueue.length = 0;
        return { type: "midi_status", ok: true, message: "Panic sent" };
      }
      if (effect.type === "sample_list_request") {
        void invokeBridge("sample_list", { dir: effect.dir }).then((entries) => {
          const safe = Array.isArray(entries)
            ? entries
              .map((e: any) => ({
                name: String(e?.name ?? ""),
                path: String(e?.path ?? ""),
                isDir: Boolean(e?.isDir ?? e?.is_dir ?? false)
              }))
              .filter((e: any) => e.name.length > 0)
            : [];
          const applied = applyStoreResult(
            state,
            {
              type: "sample_list_result",
              instrumentSlot: effect.instrumentSlot,
              sampleSlot: effect.sampleSlot,
              dir: effect.dir,
              entries: safe
            } as any,
            activeBehavior()
          );
          state = applied.state;
          publishSnapshot();
        }).catch((err) => {
          const message = err instanceof Error ? err.message : String(err);
          const applied = applyStoreResult(
            state,
            {
              type: "sample_list_error",
              instrumentSlot: effect.instrumentSlot,
              sampleSlot: effect.sampleSlot,
              dir: effect.dir,
              message
            } as any,
            activeBehavior()
          );
          state = applied.state;
          publishSnapshot();
        });
        return {
          type: "sample_list_result",
          instrumentSlot: effect.instrumentSlot,
          sampleSlot: effect.sampleSlot,
          dir: effect.dir,
          entries: []
        } as any;
      }
      if (effect.type === "sample_preview_request") {
        void invokeBridge("sample_preview", { path: effect.path }).catch((err) => {
          const applied = applyStoreResult(
            state,
            {
              type: "sample_preview_error",
              message: err instanceof Error ? err.message : String(err)
            } as any,
            activeBehavior()
          );
          state = applied.state;
          publishSnapshot();
        });
        return { type: "save_default_result", ok: true } as any;
      }
      if (effect.type === "audio_command") {
        void invokeBridge("audio_command", { command: effect.command }).catch(() => {});
        return null;
      }
      return { type: "store_error", message: "Unknown effect" };
    } catch (err) {
      return { type: "store_error", message: err instanceof Error ? err.message : "Store error" };
    }
  }

  function applyEffects(initial: PlatformEffect[]) {
    const queue = initial.slice();
    while (queue.length > 0) {
      const effect = queue.shift();
      if (!effect) break;
      const result = execEffect(effect);
      if (!result) continue;
      const applied = applyStoreResult(state, result, activeBehavior());
      state = applied.state;
      queue.push(...applied.effects);
    }
  }

  return {
    dispatch(input) {
      applyInput(input);
    },
    dispatchAction(action) {
      if (action.type === "emergency_brake") {
        const result = emergencyBrake(state);
        state = result.state;
        enqueueEvents(result.events, performance.now());
        flushDueEvents(performance.now());
        publishSnapshot();
        return;
      }
      if (action.type === "shift") {
        shiftActive = action.active;
        applyInput({ type: "button_shift", pressed: action.active });
        return;
      }
      if (action.type === "fn") {
        applyInput({ type: "button_fn", pressed: action.active });
        return;
      }
      applyInput(action.input);
    },
    start() {
      scheduler.start((nowMs, elapsedMs) => {
        // Keep MIDI bindings aligned with persisted config.
        {
          const cfg = state.runtimeConfig.midi;
          if (cfg.enabled) {
            if (cfg.outId !== midi.getSelectedOutputId()) void midi.selectOutput(cfg.outId);
            if (cfg.inId !== midi.getSelectedInputId()) void midi.selectInput(cfg.inId);
          } else {
            if (midi.getSelectedOutputId() !== null) void midi.selectOutput(null);
            if (midi.getSelectedInputId() !== null) void midi.selectInput(null);
          }
        }

        const safeElapsedMs = Math.min(elapsedMs, MAX_CATCHUP_MS);
        const prevPulse = state.transport.ppqnPulse;

        if (state.runtimeConfig.midi.syncMode === "external" && state.runtimeConfig.midi.clockInEnabled) {
          while (extMsgs.length > 0) {
            const m = extMsgs.shift();
            if (!m) break;
            const di: DeviceInput = m === "start" ? { type: "midi_start" } : m === "continue" ? { type: "midi_continue" } : { type: "midi_stop" };
            const r = routeInput(state, di, activeBehavior());
            state = r.state;
            enqueueEvents(r.events, nowMs + LOOKAHEAD_MS);
            applyEffects(r.effects);
          }
          if (extPulses > 0) {
            const pulses = extPulses;
            extPulses = 0;
            const r = routeInput(state, { type: "midi_clock", pulses }, activeBehavior());
            state = r.state;
            enqueueEvents(r.events, nowMs + LOOKAHEAD_MS);
            applyEffects(r.effects);
          }
        } else {
          const result = tick(state, activeBehavior(), safeElapsedMs / 1000);
          state = result.state;
          enqueueEvents(result.events, nowMs + LOOKAHEAD_MS);
          applyEffects(result.effects);
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
      listener(snapshotFromState(state));
      return () => listeners.delete(listener);
    },
    subscribeEvents(listener) {
      eventListeners.add(listener);
      return () => eventListeners.delete(listener);
    },
    getSnapshot() {
      return snapshotFromState(state);
    }
  };
}
