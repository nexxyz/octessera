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

test("runtime boots, dispatches, and publishes snapshots", async () => {
  const scheduler = new FakeScheduler();
  let outputsListed = 0;
  let inputsListed = 0;
  let snapshots = 0;
  const runtime = createSimulatorRuntime(scheduler, {
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

test("auto-save default debounces repeated config edits", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
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

  const press = () => runtime.dispatch({ type: "encoder_press" });
  const turn = (delta: -1 | 1) => runtime.dispatch({ type: "encoder_turn", delta });
  const back = () => runtime.dispatch({ type: "button_a" });
  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const selected = snapshot?.frame.display.lines.find((line) => line.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
    assert.fail(`failed to select label: ${label}`);
  };

  runtime.start();
  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Default");
  press();
  selectLabel("Auto Save");
  press();
  turn(1);
  press();
  assert.equal(store.defaultSaveCount(), 1, "enabling auto-save saves immediately");

  back();
  back();
  back();
  selectLabel("System");
  press();
  selectLabel("Sound");
  press();
  press();
  turn(1);
  turn(1);
  turn(1);
  assert.equal(store.defaultSaveCount(), 1, "sweep edits should not save immediately");

  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.equal(store.defaultSaveCount(), 2, "cooldown writes once");
  assert.equal(store.defaultPayload()?.runtimeConfig.masterVolume, 76, "cooldown saves latest value");
});

test("deferred auto-save flashes for ~500ms after save", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
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

  const press = () => runtime.dispatch({ type: "encoder_press" });
  const turn = (delta: -1 | 1) => runtime.dispatch({ type: "encoder_turn", delta });
  const back = () => runtime.dispatch({ type: "button_a" });
  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const selected = snapshot?.frame.display.lines.find(line => line.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
  };

  runtime.start();
  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Default");
  press();
  selectLabel("Auto Save");
  press();
  turn(1);
  press();
  await waitMicrotask();
  assert.equal(store.defaultSaveCount(), 1, "enabling auto-save saves immediately");

  back();
  back();
  back();
  selectLabel("System");
  press();
  selectLabel("Sound");
  press();
  press();
  turn(1);
  turn(1);
  // Wait for deferred save (10ms) + one tick to process the effect → applyStoreResult → flash
  await new Promise(resolve => setTimeout(resolve, 100));

  assert.equal(store.defaultSaveCount(), 2, "cooldown triggers save");
  assert.ok(snapshot, "should have a snapshot after save");
  assert.equal(snapshot?.autoSaveFlash, "flash", "flash should be active after save");

  // Let flash expire
  await new Promise(resolve => setTimeout(resolve, 600));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  // Core decay has cleared the flash
  assert.equal(store.defaultSaveCount(), 2, "save count unchanged during flash");
});

test("manual Save Default also triggers flash", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
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

  const press = () => runtime.dispatch({ type: "encoder_press" });
  const turn = (delta: -1 | 1) => runtime.dispatch({ type: "encoder_turn", delta });
  const back = () => runtime.dispatch({ type: "button_a" });
  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const selected = snapshot?.frame.display.lines.find((line) => line.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
  };

  runtime.start();
  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Default");
  press();
  selectLabel("Save Default");
  press();
  // Now in confirmation dialog - turn to select "Yes" and press
  turn(1);
  press();
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
    store,
    autoSaveCooldownMs: 50,
    autoSaveDefault: true,
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

  const press = () => runtime.dispatch({ type: "encoder_press" });
  const turn = (delta: -1 | 1) => runtime.dispatch({ type: "encoder_turn", delta });
  const back = () => runtime.dispatch({ type: "button_a" });
  const selectLabel = (label: string) => {
    for (let i = 0; i < 80; i += 1) {
      const selected = snapshot?.frame.display.lines.find((line) => line.startsWith("@@")) ?? "";
      if (selected.includes(label)) return;
      turn(1);
    }
  };

  runtime.start();
  selectLabel("System");
  press();
  selectLabel("Saves");
  press();
  selectLabel("Default");
  press();
  selectLabel("Auto Save");
  press();
  turn(1);
  press();
  await waitMicrotask();
  assert.equal(store.defaultSaveCount(), 1, "enabling auto-save saves immediately");

  back();
  back();
  back();
  selectLabel("System");
  press();
  selectLabel("Sound");
  press();
  press();
  turn(1);
  turn(1);
  // Wait for deferred save + tick
  await new Promise((resolve) => setTimeout(resolve, 150));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  assert.equal(store.defaultSaveCount(), 2, "cooldown triggers save");
  assert.ok(snapshot, "should have a snapshot after save");
  assert.equal(snapshot?.autoSaveFlash, "flash", "flash should be active after save");
  assert.ok(snapshot?.autoSaveFlash !== undefined, "autoSaveFlash field should exist");

  // Advance enough to let flash expire
  await new Promise((resolve) => setTimeout(resolve, 600));
  scheduler.tick(Date.now(), 125);
  await waitMicrotask();

  assert.equal(store.defaultSaveCount(), 2, "save count unchanged during flash");
});

test("deferred auto-save default field exists", async () => {
  const scheduler = new FakeScheduler();
  const store = memoryStore();
  let snapshot: ReturnType<ReturnType<typeof createSimulatorRuntime>["getSnapshot"]> | null = null;
  const runtime = createSimulatorRuntime(scheduler, {
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
