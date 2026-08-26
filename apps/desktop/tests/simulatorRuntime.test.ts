import test from 'node:test';
import assert from 'node:assert/strict';
import {
  GRID_HEIGHT,
  GRID_WIDTH,
  RED_COLOR,
  createOledFrameRevision,
} from '@octessera/device-contracts';
import {
  createSimulatorRuntime,
  shouldApplyRuntimeBatch,
} from '../src/runtime/simulatorRuntime';
import { scaleNeoKeyLeds } from '../src/runtime/simulatorSnapshot';
import type { RuntimeRunnerMessage } from '@octessera/device-contracts';
import type { RuntimeScheduler } from '../src/runtime/runtimeScheduler';

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

function snapshotMessage(
  options: {
    audioConfigRevision?: number;
    instruments?: unknown[];
    mixer?: unknown;
    masterVolume?: number;
    ledsDimmed?: boolean;
    displayOff?: boolean;
    eventDotOn?: boolean;
    transportFlash?: 'none' | 'beat' | 'measure';
  } = {},
) {
  return {
    type: 'snapshot' as const,
    snapshot: {
      leds: {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        rgb: Array.from({ length: GRID_WIDTH * GRID_HEIGHT * 3 }, () => 0),
        active: Array.from({ length: GRID_WIDTH * GRID_HEIGHT }, () => false),
      },
      transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
      display: {
        page: 'boot',
        bodyLayout: 'rows',
        title: 'Boot',
        lines: [],
        editing: false,
        off: options.displayOff,
      },
      activeBehavior: 'life',
      gridInteraction: 'paint' as const,
      neoKeyLeds: {
        back: [221, 130, 205],
        space: [221, 130, 205],
        shift: [67, 68, 71],
        fn: [67, 68, 71],
      },
      eventDotOn: options.eventDotOn ?? false,
      transportIcon: 'stop' as const,
      transportFlash: options.transportFlash ?? 'none',
      settings: {
        displayBrightness: 75,
        buttonBrightness: 75,
        masterVolume: options.masterVolume ?? 73,
        voiceStealingMode: 'auto-balanced' as const,
        instruments: options.instruments ?? [],
        mixer: options.mixer ?? { buses: [] },
        panPositions: 33,
        audioConfigRevision: options.audioConfigRevision,
        ledsDimmed: options.ledsDimmed,
        autoSaveFlash: 'none' as const,
        stopLatched: false,
        shiftHeld: false,
        fnHeld: false,
        combinedModifierHeld: false,
        midi: {
          enabled: false,
          outId: null,
          inId: null,
          syncMode: 'internal' as const,
          clockOutEnabled: false,
          clockInEnabled: false,
        },
      },
      oledFrameRevision: createOledFrameRevision(1),
    },
  };
}

test('runtime requires Tauri native runtime or injected dispatch', () => {
  assert.throws(
    () => createSimulatorRuntime(new FakeScheduler()),
    /requires Tauri native runtime or an injected native dispatch/,
  );
});

test('fault-bearing runtime batches bypass stale display suppression', () => {
  const fault = {
    type: 'runtime_status' as const,
    status: {
      state: 'error' as const,
      transport: 'stopped' as const,
      currentPpqnPulse: 0,
      pendingResync: false,
      syncSource: 'internal' as const,
      error: {
        domain: 'audio' as const,
        code: 'operation_failed' as const,
        operation: 'audio_thread' as const,
        recovery: 'stop_and_silence' as const,
        message: 'audio stopped',
      },
    },
  } satisfies RuntimeRunnerMessage;

  assert.equal(shouldApplyRuntimeBatch(4, 5, 10, 100, [fault]), true);
  assert.equal(
    shouldApplyRuntimeBatch(4, 5, 10, 100, [snapshotMessage()]),
    false,
  );
});

function sparseAudioSnapshotMessage(
  options: { audioConfigRevision?: number; masterVolume?: number } = {},
) {
  return {
    type: 'snapshot' as const,
    snapshot: {
      leds: {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        rgb: Array.from({ length: GRID_WIDTH * GRID_HEIGHT * 3 }, () => 0),
        active: Array.from({ length: GRID_WIDTH * GRID_HEIGHT }, () => false),
      },
      transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
      display: {
        page: 'boot',
        bodyLayout: 'rows',
        title: 'Boot',
        lines: [],
        editing: false,
      },
      activeBehavior: 'life',
      gridInteraction: 'paint' as const,
      neoKeyLeds: {
        back: [221, 130, 205],
        space: [221, 130, 205],
        shift: [67, 68, 71],
        fn: [67, 68, 71],
      },
      eventDotOn: false,
      transportIcon: 'stop' as const,
      transportFlash: 'none' as const,
      settings: {
        displayBrightness: 75,
        buttonBrightness: 75,
        masterVolume: options.masterVolume ?? 73,
        voiceStealingMode: 'auto-balanced' as const,
        audioConfigRevision: options.audioConfigRevision,
        autoSaveFlash: 'none' as const,
        stopLatched: false,
        shiftHeld: false,
        fnHeld: false,
        combinedModifierHeld: false,
        midi: {
          enabled: false,
          outId: null,
          inId: null,
          syncMode: 'internal' as const,
          clockOutEnabled: false,
          clockInEnabled: false,
        },
      },
      oledFrameRevision: createOledFrameRevision(1),
    },
  };
}

test('runtime dispatches hardware input through native dispatch', async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    },
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(seen.at(-1), {
    type: 'device_input',
    input: { type: 'grid_press', x: 1, y: 2 },
  });
});

test('runtime start requests an initial native snapshot', async () => {
  const scheduler = new FakeScheduler();
  const seen: any[] = [];
  let snapshots = 0;
  const runtime = createSimulatorRuntime(scheduler, {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    },
  });
  runtime.subscribe(() => (snapshots += 1));

  runtime.start();
  scheduler.tick(1000, 16);
  await waitMicrotask();

  assert.equal(seen[0].type, 'transport_pulse_step');
  assert.ok(snapshots >= 2);
});

test('runtime coalesces encoder turn bursts', async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    },
  });

  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: -1 });
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: 2 },
    },
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: -1 },
    },
  ]);
});

test('runtime preserves encoder direction reversals for main and aux', async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return [snapshotMessage()];
    },
  });

  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: -1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'aux2', delta: -1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'aux2', delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));
  await waitMicrotask();
  await waitMicrotask();

  assert.deepEqual(seen, [
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: 1 },
    },
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: -1 },
    },
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'aux2', delta: -1 },
    },
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'aux2', delta: 1 },
    },
  ]);
});

test('runtime coalesces encoder turns while a dispatch is in flight', async () => {
  const seen: any[] = [];
  const releaseFirst: Array<() => void> = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: (message) =>
      new Promise((resolve) => {
        seen.push(message);
        if (seen.length === 1) {
          releaseFirst.push(() => resolve([snapshotMessage()]));
          return;
        }
        resolve([snapshotMessage()]);
      }),
  });

  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));
  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  runtime.dispatch({ type: 'encoder_turn', id: 'main', delta: 1 });
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: 1 },
    },
  ]);

  releaseFirst[0]!();
  await waitMicrotask();
  await waitMicrotask();
  await new Promise((resolve) => setTimeout(resolve, 12));

  assert.deepEqual(seen, [
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: 1 },
    },
    {
      type: 'device_input',
      input: { type: 'encoder_turn', id: 'main', delta: 2 },
    },
  ]);
});

test('runtime preserves audio config refs while revision is unchanged', async () => {
  const instruments = [{ type: 'synth', value: 1 }];
  const mixer = { buses: [{ name: 'bus' }] };
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [
      snapshotMessage({
        audioConfigRevision: 1,
        instruments,
        mixer,
        masterVolume: 80,
      }),
    ],
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();
  const first = runtime.getSnapshot();
  runtime.dispatch({ type: 'grid_press', x: 2, y: 3 });
  await waitMicrotask();
  const second = runtime.getSnapshot();

  assert.equal(second.instruments, first.instruments);
  assert.equal(second.mixer, first.mixer);
  assert.equal(second.masterVolume, 80);
});

test('runtime preserves cached audio config when snapshots omit unchanged audio payloads', async () => {
  let dispatchCount = 0;
  const instruments = [{ type: 'synth', value: 1 }];
  const mixer = { buses: [{ name: 'bus' }] };
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => {
      dispatchCount += 1;
      return [
        dispatchCount === 1
          ? snapshotMessage({
              audioConfigRevision: 1,
              instruments,
              mixer,
              masterVolume: 80,
            })
          : sparseAudioSnapshotMessage({
              audioConfigRevision: 1,
              masterVolume: 80,
            }),
      ];
    },
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();
  const first = runtime.getSnapshot();
  runtime.dispatch({ type: 'grid_press', x: 2, y: 3 });
  await waitMicrotask();
  const second = runtime.getSnapshot();

  assert.equal(second.instruments, first.instruments);
  assert.equal(second.mixer, first.mixer);
  assert.equal(second.panPositions, first.panPositions);
});

test('runtime applies native transient presentation fields', async () => {
  let transient = true;
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (): Promise<RuntimeRunnerMessage[]> => [
      snapshotMessage({
        eventDotOn: transient,
        transportFlash: transient ? 'measure' : 'none',
      }),
    ],
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  const snapshot = runtime.getSnapshot();
  assert.equal(snapshot.frame.eventDotOn, true);
  assert.equal(snapshot.frame.transportFlash, 'measure');

  transient = false;
  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();
  assert.equal(runtime.getSnapshot().frame.eventDotOn, false);
});

test('runtime applies native ledsDimmed to desktop NeoKey LEDs', async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [snapshotMessage({ ledsDimmed: true })],
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(
    runtime.getSnapshot().neoKeyLeds.space,
    RED_COLOR.map((channel) => Math.floor((channel * 600 + 5000) / 10000)),
  );
});

test('display off does not dim desktop NeoKey LEDs', async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => [
      snapshotMessage({ displayOff: true, ledsDimmed: false }),
    ],
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(
    runtime.getSnapshot().neoKeyLeds.space,
    RED_COLOR.map((channel) => Math.floor((channel * 7500 + 5000) / 10000)),
  );
});

test('desktop NeoKey basis-point scaling matches normal and dimmed brightness', () => {
  const leds = {
    back: [255, 128, 1] as [number, number, number],
    space: [255, 128, 1] as [number, number, number],
    shift: [255, 128, 1] as [number, number, number],
    fn: [255, 128, 1] as [number, number, number],
  };
  for (const brightness of [0, 10, 75, 100]) {
    for (const dimmed of [false, true]) {
      const basisPoints = dimmed
        ? brightness === 0
          ? 0
          : Math.max(brightness * 8, 400)
        : brightness * 100;
      const expected = [255, 128, 1].map((channel) =>
        Math.floor((channel * basisPoints + 5000) / 10000),
      );
      assert.deepEqual(
        scaleNeoKeyLeds(leds, brightness, dimmed).back,
        expected,
      );
    }
  }
});
