import test from "node:test";
import assert from "node:assert/strict";
import { GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH, RED_COLOR } from "@octessera/device-contracts";
import { createSimulatorRuntime } from "../src/runtime/simulatorRuntime";
import type { RuntimeRunnerMessage } from "@octessera/device-contracts";
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

function waitMicrotask(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function snapshotMessage(options: { audioConfigRevision?: number; instruments?: unknown[]; mixer?: unknown; masterVolume?: number; ledsDimmed?: boolean; displayOff?: boolean } = {}) {
  return {
    type: "snapshot" as const,
    snapshot: {
      oled: { width: OLED_WIDTH, height: OLED_HEIGHT, format: "rgb565be" as const, pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2) },
      leds: { width: GRID_WIDTH, height: GRID_HEIGHT, rgb: Array.from({ length: GRID_WIDTH * GRID_HEIGHT * 3 }, () => 0), active: Array.from({ length: GRID_WIDTH * GRID_HEIGHT }, () => false) },
      transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
      display: { page: "boot", title: "Boot", lines: [], editing: false, off: options.displayOff },
      activeBehavior: "life",
      gridInteraction: "paint" as const,
      settings: {
        displayBrightness: 75,
        buttonBrightness: 75,
        masterVolume: options.masterVolume ?? 73,
        voiceStealingMode: "auto-balanced" as const,
        instruments: options.instruments ?? [],
        mixer: options.mixer ?? { buses: [] },
        panPositions: 33,
        audioConfigRevision: options.audioConfigRevision,
        ledsDimmed: options.ledsDimmed,
        autoSaveFlash: "none" as const,
        transportFlash: "none" as const,
        stopLatched: false,
        shiftHeld: false,
        fnHeld: false,
        combinedModifierHeld: false,
        midi: { enabled: false, outId: null, inId: null, syncMode: "internal" as const, clockOutEnabled: false, clockInEnabled: false }
      }
    }
  };
}

test("runtime requires Tauri native runtime or injected dispatch", () => {
  assert.throws(
    () => createSimulatorRuntime(new FakeScheduler()),
    /requires Tauri native runtime or an injected native dispatch/,
  );
});

function sparseAudioSnapshotMessage(options: { audioConfigRevision?: number; masterVolume?: number } = {}) {
  return {
    type: "snapshot" as const,
    snapshot: {
      oled: { width: OLED_WIDTH, height: OLED_HEIGHT, format: "rgb565be" as const, pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2) },
      leds: { width: GRID_WIDTH, height: GRID_HEIGHT, rgb: Array.from({ length: GRID_WIDTH * GRID_HEIGHT * 3 }, () => 0), active: Array.from({ length: GRID_WIDTH * GRID_HEIGHT }, () => false) },
      transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
      display: { page: "boot", title: "Boot", lines: [], editing: false },
      activeBehavior: "life",
      gridInteraction: "paint" as const,
      settings: {
        displayBrightness: 75,
        buttonBrightness: 75,
        masterVolume: options.masterVolume ?? 73,
        voiceStealingMode: "auto-balanced" as const,
        audioConfigRevision: options.audioConfigRevision,
        autoSaveFlash: "none" as const,
        transportFlash: "none" as const,
        stopLatched: false,
        shiftHeld: false,
        fnHeld: false,
        combinedModifierHeld: false,
        midi: { enabled: false, outId: null, inId: null, syncMode: "internal" as const, clockOutEnabled: false, clockInEnabled: false }
      }
    }
  };
}

test("runtime dispatches hardware input through native dispatch", async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    }
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(seen.at(-1), { type: "device_input", input: { type: "grid_press", x: 1, y: 2 } });
});

test("runtime start requests an initial native snapshot", async () => {
  const scheduler = new FakeScheduler();
  const seen: any[] = [];
  let snapshots = 0;
  const runtime = createSimulatorRuntime(scheduler, {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    }
  });
  runtime.subscribe(() => snapshots += 1);

  runtime.start();
  scheduler.tick(1000, 16);
  await waitMicrotask();

  assert.equal(seen[0].type, "transport_pulse_step");
  assert.ok(snapshots >= 2);
});

test("runtime coalesces encoder turn bursts", async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    }
  });

  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", id: "main", delta: -1 });
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: 2 } },
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: -1 } }
  ]);
});

test("runtime preserves encoder direction reversals for main and aux", async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    }
  });

  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", id: "main", delta: -1 });
  runtime.dispatch({ type: "encoder_turn", id: "aux2", delta: -1 });
  runtime.dispatch({ type: "encoder_turn", id: "aux2", delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));
  await waitMicrotask();
  await waitMicrotask();

  assert.deepEqual(seen, [
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: 1 } },
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: -1 } },
    { type: "device_input", input: { type: "encoder_turn", id: "aux2", delta: -1 } },
    { type: "device_input", input: { type: "encoder_turn", id: "aux2", delta: 1 } }
  ]);
});

test("runtime coalesces encoder turns while a dispatch is in flight", async () => {
  const seen: any[] = [];
  const releaseFirst: Array<() => void> = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: (message) => new Promise((resolve) => {
      seen.push(message);
      if (seen.length === 1) {
        releaseFirst.push(() => resolve([snapshotMessage()]));
        return;
      }
      resolve([snapshotMessage()]);
    })
  });

  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));
  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  runtime.dispatch({ type: "encoder_turn", id: "main", delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [{ type: "device_input", input: { type: "encoder_turn", id: "main", delta: 1 } }]);

  releaseFirst[0]!();
  await waitMicrotask();
  await waitMicrotask();
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: 1 } },
    { type: "device_input", input: { type: "encoder_turn", id: "main", delta: 2 } }
  ]);
});

test("runtime preserves audio config refs while revision is unchanged", async () => {
  const instruments = [{ type: "synth", value: 1 }];
  const mixer = { buses: [{ name: "bus" }] };
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [snapshotMessage({ audioConfigRevision: 1, instruments, mixer, masterVolume: 80 })]
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();
  const first = runtime.getSnapshot();
  runtime.dispatch({ type: "grid_press", x: 2, y: 3 });
  await waitMicrotask();
  const second = runtime.getSnapshot();

  assert.equal(second.instruments, first.instruments);
  assert.equal(second.mixer, first.mixer);
  assert.equal(second.masterVolume, 80);
});

test("runtime preserves cached audio config when snapshots omit unchanged audio payloads", async () => {
  let dispatchCount = 0;
  const instruments = [{ type: "synth", value: 1 }];
  const mixer = { buses: [{ name: "bus" }] };
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => {
      dispatchCount += 1;
      return [dispatchCount === 1
        ? snapshotMessage({ audioConfigRevision: 1, instruments, mixer, masterVolume: 80 })
        : sparseAudioSnapshotMessage({ audioConfigRevision: 1, masterVolume: 80 })];
    }
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();
  const first = runtime.getSnapshot();
  runtime.dispatch({ type: "grid_press", x: 2, y: 3 });
  await waitMicrotask();
  const second = runtime.getSnapshot();

  assert.equal(second.instruments, first.instruments);
  assert.equal(second.mixer, first.mixer);
  assert.equal(second.panPositions, first.panPositions);
});

test("runtime applies native ui pulses for indicators", async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (): Promise<RuntimeRunnerMessage[]> => [
      snapshotMessage(),
      { type: "ui_pulse", pulse: { type: "trigger_pulse", durationMs: 40 } },
      { type: "ui_pulse", pulse: { type: "transport_flash", flash: "measure", durationMs: 40 } }
    ]
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();

  const snapshot = runtime.getSnapshot();
  assert.equal((snapshot.frame as any).eventDotOn, true);
  assert.equal((snapshot.frame as any).transportFlash, "measure");

  await new Promise((resolve) => setTimeout(resolve, 50));
  const after = runtime.getSnapshot();
  assert.notEqual((after.frame as any).eventDotOn, true);
});

test("runtime applies native ledsDimmed to desktop NeoKey LEDs", async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [snapshotMessage({ ledsDimmed: true })]
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(runtime.getSnapshot().neoKeyLeds.space, RED_COLOR.map((channel) => Math.round(channel * 0.22 * 0.75)));
});

test("display off does not dim desktop NeoKey LEDs", async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [snapshotMessage({ displayOff: true, ledsDimmed: false })]
  });

  runtime.dispatch({ type: "grid_press", x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(runtime.getSnapshot().neoKeyLeds.space, RED_COLOR.map((channel) => Math.round(channel * 0.75)));
});
