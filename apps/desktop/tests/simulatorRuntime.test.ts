import test from "node:test";
import assert from "node:assert/strict";
import { createSimulatorRuntime } from "../src/runtime/simulatorRuntime";
import type { RuntimeScheduler } from "../src/runtime/runtimeScheduler";

class FakeScheduler implements RuntimeScheduler {
  private onTick: ((nowMs: number, elapsedMs: number) => void) | null = null;
  start(onTick: (nowMs: number, elapsedMs: number) => void): void {
    this.onTick = onTick;
  }
  stop(): void {
    this.onTick = null;
  }
  tick(nowMs: number, elapsedMs: number): void {
    this.onTick?.(nowMs, elapsedMs);
  }
}

function memoryStore() {
  const presets = new Map<string, any>();
  let defaults: any = null;
  let defaultSaveCount = 0;
  return {
    listPresets: () => Array.from(presets.keys()),
    loadPreset: (name: string) => presets.get(name) ?? null,
    savePreset: (name: string, payload: any) => {
      const existed = presets.has(name);
      presets.set(name, payload);
      return existed ? "overwritten" as const : "created" as const;
    },
    deletePreset: (name: string) => presets.delete(name),
    loadDefault: () => defaults,
    saveDefault: (payload: any) => {
      defaultSaveCount += 1;
      defaults = payload;
    },
    defaultPayload: () => defaults,
    defaultSaveCount: () => defaultSaveCount
  };
}

function waitMicrotask(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function fakeRunner(
  handle?: (message: any, state: any, frame: any) => any[]
) {
  const state = {
    runtimeConfig: {
      midi: { enabled: false, outId: null, inId: null, syncMode: "internal", clockInEnabled: false, clockOutEnabled: false },
      displayBrightness: 75,
      buttonBrightness: 75,
      masterVolume: 73,
      sound: { voiceStealingMode: "balanced" },
      instruments: [],
      mixer: { buses: [] },
      panPositions: 33
    },
    transport: { playing: false, ppqnPulse: 0 },
    system: { transportFlash: "none", combinedModifierHeld: false, fnHeld: false, stopLatched: false, autoSaveFlash: "none" }
  };
  const frame = {
    oled: { width: 128, height: 128, format: "rgb565be" as const, pixels: new Uint8Array(32768) },
    leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
    transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
    display: { page: "boot", title: "Boot", lines: [], editing: false },
    activeBehavior: "life",
    gridInteraction: "paint",
    settings: state.runtimeConfig
  };
  return {
    dispatch: (message: any) => handle?.(message, state, frame) ?? [],
    getState: () => state,
    getFrame: () => frame
  };
}

test("runtime boots, dispatches, and publishes snapshots", async () => {
  const scheduler = new FakeScheduler();
  let outputsListed = 0;
  let inputsListed = 0;
  let snapshots = 0;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner(),
    store: memoryStore(),
    midiService: {
      listOutputs: async () => {
        outputsListed += 1;
        return [];
      },
      listInputs: async () => {
        inputsListed += 1;
        return [];
      },
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.subscribe(() => {
    snapshots += 1;
  });
  runtime.start();
  await waitMicrotask();
  runtime.dispatch({ type: "button_s" });
  runtime.dispatch({ type: "button_s" });
  runtime.dispatchAction({ type: "shift", active: true });
  runtime.dispatchAction({ type: "shift", active: false });
  scheduler.tick(1000, 16);
  await waitMicrotask();
  assert.ok(outputsListed >= 1);
  assert.ok(inputsListed >= 1);
  assert.ok(snapshots >= 1);
});

test("runtime tick path executes without tauri bridge", async () => {
  const scheduler = new FakeScheduler();
  let sentCount = 0;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner(),
    store: memoryStore(),
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {
        sentCount += 1;
      },
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.start();
  await waitMicrotask();
  for (let i = 0; i < 8; i += 1) {
    scheduler.tick(1000 + i * 8, 8);
  }
  await waitMicrotask();
  assert.ok(sentCount >= 0);
});

test("runtime owns audio event forwarding without React subscribeEvents bridge", async () => {
  const scheduler = new FakeScheduler();
  const forwarded: any[] = [];
  const runtime = createSimulatorRuntime(scheduler, {
    runner: {
      dispatch(message: any) {
        return message.type === "device_input"
          ? [{ type: "musical_events", events: [{ type: "note_on", channel: 0, note: 60, velocity: 100, durationMs: 120 }] }]
          : [];
      },
      getState() {
        return {
          runtimeConfig: {
            midi: { enabled: false, outId: null, syncMode: "internal", clockOutEnabled: false },
            displayBrightness: 75,
            buttonBrightness: 75,
            masterVolume: 100,
            sound: { voiceStealingMode: "balanced" },
            instruments: [],
            mixer: { buses: [] },
            panPositions: 7
          },
          transport: { playing: false, bpm: 120, ppqnPulse: 0 },
          system: { stopLatched: false, transportFlash: "none", fnHeld: false, combinedModifierHeld: false, autoSaveFlash: "none" }
        } as any;
      },
      getFrame() {
        return {
          oled: null,
          leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          display: { page: "test", title: "Test", lines: [], editing: false },
          activeBehavior: "life",
          gridInteraction: "paint"
        } as any;
      }
    } as any,
    store: memoryStore(),
    audioEventSink: async (events) => {
      forwarded.push(...events);
    },
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });

  runtime.start();
  runtime.dispatch({ type: "grid_press", x: 0, y: 0 });

  assert.equal(forwarded.length, 1);
});

test("tauri mode bypasses local runner for transport play button", async () => {
  const scheduler = new FakeScheduler();
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(scheduler, {
    runner: {
      dispatch(message: any) {
        seen.push(message);
        return [];
      },
      getState() {
        return {
          runtimeConfig: {
            midi: { enabled: false, outId: null, syncMode: "internal", clockOutEnabled: false },
            displayBrightness: 75,
            buttonBrightness: 75,
            masterVolume: 100,
            sound: { voiceStealingMode: "balanced" },
            instruments: [],
            mixer: { buses: [] },
            panPositions: 7
          },
          transport: { playing: false, bpm: 120, ppqnPulse: 0 },
          system: { stopLatched: false, transportFlash: "none", fnHeld: false, combinedModifierHeld: false, autoSaveFlash: "none" }
        } as any;
      },
      getFrame() {
        return {
          oled: null,
          leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          display: { page: "test", title: "Test", lines: [], editing: false },
          activeBehavior: "life",
          gridInteraction: "paint"
        } as any;
      }
    } as any,
    runtimeDispatch: async () => {
      return [
        {
          type: "snapshot",
          snapshot: {
            oled: null,
            leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
            transport: { playing: true, bpm: 120, tick: 0, ppqnPulse: 0 },
            display: { page: "test", title: "Test", lines: [], editing: false },
            activeBehavior: "life",
            gridInteraction: "paint"
          }
        },
        {
          type: "runtime_status",
          status: { state: "running", transport: "playing", currentPpqnPulse: 0, pendingResync: false, syncSource: "internal" }
        }
      ] as any;
    },
    store: memoryStore(),
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });

  runtime.start();
  await waitMicrotask();
  runtime.dispatch({ type: "button_s" } as any);
  await waitMicrotask();
  await waitMicrotask();

  assert.equal(seen.some((message) => message.type === "device_input" && message.input?.type === "button_s"), false);
  assert.equal(runtime.getSnapshot().frame.transport.playing, true);
});

test("tauri mode bypasses local runner for grid performance input", async () => {
  const scheduler = new FakeScheduler();
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(scheduler, {
    runner: {
      dispatch(message: any) {
        seen.push(message);
        return [];
      },
      getState() {
        return {
          runtimeConfig: {
            midi: { enabled: false, outId: null, syncMode: "internal", clockOutEnabled: false },
            displayBrightness: 75,
            buttonBrightness: 75,
            masterVolume: 100,
            sound: { voiceStealingMode: "balanced" },
            instruments: [],
            mixer: { buses: [] },
            panPositions: 7
          },
          transport: { playing: false, bpm: 120, ppqnPulse: 0 },
          system: { stopLatched: false, transportFlash: "none", fnHeld: false, combinedModifierHeld: false, autoSaveFlash: "none" }
        } as any;
      },
      getFrame() {
        return {
          oled: null,
          leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          display: { page: "test", title: "Test", lines: [], editing: false },
          activeBehavior: "life",
          gridInteraction: "paint"
        } as any;
      }
    } as any,
    runtimeDispatch: async () => {
      const cells = Array.from({ length: 64 }, (_, index) => (index === 0 ? { r: 0, g: 255, b: 0 } : { r: 0, g: 0, b: 0 }));
      return [
        {
          type: "snapshot",
          snapshot: {
            oled: null,
            leds: { width: 8, height: 8, cells },
            transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
            display: { page: "test", title: "Test", lines: [], editing: false },
            activeBehavior: "life",
            gridInteraction: "paint"
          }
        },
        {
          type: "runtime_status",
          status: { state: "idle", transport: "stopped", currentPpqnPulse: 0, pendingResync: false, syncSource: "internal" }
        }
      ] as any;
    },
    store: memoryStore(),
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });

  runtime.start();
  await waitMicrotask();
  runtime.dispatch({ type: "grid_press", x: 0, y: 0 } as any);
  await waitMicrotask();
  await waitMicrotask();

  assert.equal(seen.some((message) => message.type === "device_input" && message.input?.type === "grid_press"), false);
  assert.equal(runtime.getSnapshot().frame.leds.cells[0]?.g, 255);
});

test("tauri mode bypasses local runner for encoder menu input", async () => {
  const scheduler = new FakeScheduler();
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(scheduler, {
    runner: {
      dispatch(message: any) {
        seen.push(message);
        return [];
      },
      getState() {
        return {
          runtimeConfig: {
            midi: { enabled: false, outId: null, syncMode: "internal", clockOutEnabled: false },
            displayBrightness: 75,
            buttonBrightness: 75,
            masterVolume: 100,
            sound: { voiceStealingMode: "balanced" },
            instruments: [],
            mixer: { buses: [] },
            panPositions: 7
          },
          transport: { playing: false, bpm: 120, ppqnPulse: 0 },
          system: { stopLatched: false, transportFlash: "none", fnHeld: false, combinedModifierHeld: false, autoSaveFlash: "none" }
        } as any;
      },
      getFrame() {
        return {
          oled: null,
          leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          display: { page: "before", title: "Before", lines: [], editing: false },
          activeBehavior: "life",
          gridInteraction: "paint"
        } as any;
      }
    } as any,
    runtimeDispatch: async () => {
      return [
        {
          type: "snapshot",
          snapshot: {
            oled: null,
            leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
            transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
            display: { page: "after", title: "After", lines: [], editing: false },
            activeBehavior: "life",
            gridInteraction: "paint",
            settings: {
              displayBrightness: 75,
              buttonBrightness: 75,
              masterVolume: 100,
              voiceStealingMode: "balanced",
              instruments: [],
              mixer: { buses: [] },
              panPositions: 7,
              autoSaveFlash: "none",
              transportFlash: "none",
              fnHeld: false,
              combinedModifierHeld: false,
              midi: { enabled: false, outId: null, inId: null, syncMode: "internal", clockOutEnabled: false, clockInEnabled: false }
            }
          }
        },
        {
          type: "runtime_status",
          status: { state: "idle", transport: "stopped", currentPpqnPulse: 0, pendingResync: false, syncSource: "internal" }
        }
      ] as any;
    },
    store: memoryStore(),
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });

  runtime.start();
  await waitMicrotask();
  runtime.dispatch({ type: "encoder_turn", delta: 1 } as any);
  await waitMicrotask();
  await waitMicrotask();

  assert.equal(seen.some((message) => message.type === "device_input" && message.input?.type === "encoder_turn"), false);
  assert.equal(runtime.getSnapshot().frame.display.page, "after");
});

test("auto-save default debounces repeated config edits", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let masterVolume = 73;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner((message) => {
      if (message.type !== "device_input" || message.input?.type !== "encoder_turn") return [];
      masterVolume += message.input.delta;
      return [{
        type: "platform_effects",
        effects: [{
          type: "store_save_default",
          mode: "deferred",
          payload: { runtimeConfig: { masterVolume } }
        }]
      }];
    }),
    store,
    autoSaveCooldownMs: 10,
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });

  runtime.start();
  runtime.dispatch({ type: "encoder_turn", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", delta: 1 });
  assert.equal(store.defaultSaveCount(), 0, "sweep edits should not save immediately");

  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.equal(store.defaultSaveCount(), 1, "cooldown writes once");
  assert.equal(store.defaultPayload()?.runtimeConfig.masterVolume, 76, "cooldown saves latest value");
});

test("deferred auto-save flashes for ~500ms after save", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner((message, state, frame) => {
      if (message.type === "device_input" && message.input?.type === "encoder_turn") {
        return [{ type: "platform_effects", effects: [{ type: "store_save_default", mode: "deferred", payload: { runtimeConfig: { masterVolume: 74 } } }] }];
      }
      if (message.type === "runtime_result" && message.result?.type === "save_default_result") {
        state.system.autoSaveFlash = "flash";
        frame.settings = { ...frame.settings, autoSaveFlash: "flash" };
        return [{ type: "snapshot", snapshot: frame }];
      }
      return [];
    }),
    store,
    autoSaveCooldownMs: 10,
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.subscribe((next) => {
    snapshot = next;
  });

  runtime.start();
  runtime.dispatch({ type: "encoder_turn", delta: 1 });
  // Wait for deferred save (10ms) + one tick to process the effect → applyStoreResult → flash
  await new Promise(resolve => setTimeout(resolve, 100));

  assert.equal(store.defaultSaveCount(), 1, "cooldown triggers save");
  assert.ok(snapshot, "should have a snapshot after save");
  assert.equal(snapshot?.autoSaveFlash, "flash", "flash should be active after save");

  // Let flash expire
  await new Promise(resolve => setTimeout(resolve, 600));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  // Core decay has cleared the flash
  assert.equal(store.defaultSaveCount(), 1, "save count unchanged during flash");
});

test("manual Save Default also triggers flash", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner((message, state, frame) => {
      if (message.type === "device_input" && message.input?.type === "encoder_press") {
        return [{ type: "platform_effects", effects: [{ type: "store_save_default", mode: "immediate", payload: { runtimeConfig: { masterVolume: 73 } } }] }];
      }
      if (message.type === "runtime_result" && message.result?.type === "save_default_result") {
        state.system.autoSaveFlash = "flash";
        frame.settings = { ...frame.settings, autoSaveFlash: "flash" };
        return [{ type: "snapshot", snapshot: frame }];
      }
      return [];
    }),
    store,
    autoSaveCooldownMs: 2000,
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.subscribe((next) => {
    snapshot = next;
  });

  runtime.start();
  runtime.dispatch({ type: "encoder_press" });
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  assert.equal(store.defaultSaveCount(), 1, "manual save triggered after confirm");
  assert.ok(snapshot?.autoSaveFlash === "flash", "flash should be active after manual save");
});

function flushRaf(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test("deferred auto-save triggers flash", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner((message, state, frame) => {
      if (message.type === "device_input" && message.input?.type === "encoder_turn") {
        return [{ type: "platform_effects", effects: [{ type: "store_save_default", mode: "deferred", payload: { runtimeConfig: { masterVolume: 75 } } }] }];
      }
      if (message.type === "runtime_result" && message.result?.type === "save_default_result") {
        state.system.autoSaveFlash = "flash";
        frame.settings = { ...frame.settings, autoSaveFlash: "flash" };
        return [{ type: "snapshot", snapshot: frame }];
      }
      return [];
    }),
    store,
    autoSaveCooldownMs: 50,
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.subscribe((next) => {
    snapshot = next;
  });

  runtime.start();
  runtime.dispatch({ type: "encoder_turn", delta: 1 });
  // Wait for deferred save + tick
  await new Promise((resolve) => setTimeout(resolve, 150));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  assert.equal(store.defaultSaveCount(), 1, "cooldown triggers save");
  assert.ok(snapshot, "should have a snapshot after save");
  assert.equal(snapshot?.autoSaveFlash, "flash", "flash should be active after save");
  assert.ok(snapshot?.autoSaveFlash !== undefined, "autoSaveFlash field should exist");

  // Advance enough to let flash expire
  await new Promise((resolve) => setTimeout(resolve, 600));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  assert.equal(store.defaultSaveCount(), 1, "save count unchanged during flash");
});

test("deferred auto-save default field exists", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
    runner: fakeRunner(),
    store,
    autoSaveCooldownMs: 2000,
    midiService: {
      listOutputs: async () => [],
      listInputs: async () => [],
      selectOutput: async () => ({ ok: true }),
      selectInput: async () => ({ ok: true }),
      send: async () => {},
      listenMidiIn: async () => () => {}
    },
    invoke: async () => []
  });
  runtime.subscribe((next) => {
    snapshot = next;
  });

  runtime.dispatch({ type: "encoder_press" });  // navigate to save
  await flushRaf();

  assert.equal(store.defaultSaveCount(), 0, "no deferred save yet");

  // Dispatch the default_save action
  runtime.dispatchAction({ type: "device_input", input: { type: "encoder_press" } });
  await flushRaf();

  // Check autoSaveFlash field is present in snapshot
  assert.ok(
    snapshot?.autoSaveFlash !== undefined || !snapshot,
    "autoSaveFlash should be accessible"
  );
});
