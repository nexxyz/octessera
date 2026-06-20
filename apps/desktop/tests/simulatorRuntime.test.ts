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

function waitMicrotask(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function snapshotMessage(options: { audioConfigRevision?: number; instruments?: unknown[]; mixer?: unknown; masterVolume?: number } = {}) {
  return {
    type: "snapshot" as const,
    snapshot: {
      oled: { width: 128, height: 128, format: "rgb565be" as const, pixels: new Uint8Array(32768) },
      leds: { width: 8, height: 8, cells: Array.from({ length: 64 }, () => ({ r: 0, g: 0, b: 0 })) },
      transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
      display: { page: "boot", title: "Boot", lines: [], editing: false },
      activeBehavior: "life",
      gridInteraction: "paint" as const,
      settings: {
        displayBrightness: 75,
        buttonBrightness: 75,
        masterVolume: options.masterVolume ?? 73,
        voiceStealingMode: "balanced" as const,
        instruments: options.instruments ?? [],
        mixer: options.mixer ?? { buses: [] },
        panPositions: 33,
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

  assert.deepEqual(seen, [{ type: "device_input", input: { type: "encoder_turn", id: "main", delta: 1 } }]);
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
