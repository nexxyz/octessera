import test from 'node:test';
import assert from 'node:assert/strict';
import {
  GRID_HEIGHT,
  GRID_WIDTH,
  MAGENTA_COLOR,
  createOledFrameRevision,
} from '@octessera/device-contracts';
import {
  createSimulatorRuntime,
  shouldApplyRuntimeBatch,
} from '../src/runtime/simulatorRuntime';
import { scaleNeoKeyLeds } from '../src/runtime/simulatorSnapshot';
import type {
  NeoKeyLeds,
  RuntimeRunnerMessage,
} from '@octessera/device-contracts';
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
    ledsDimmed?: boolean;
    displayOff?: boolean;
    eventDotLevel?: number;
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
        bodyLayout: 'rows' as const,
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
      } satisfies NeoKeyLeds,
      eventDotLevel: options.eventDotLevel ?? 0,
      transportIcon: 'stop' as const,
      transportFlash: options.transportFlash ?? ('none' as const),
      settings: {
        displayBrightness: 12,
        buttonBrightness: 75,
        masterVolume: 73,
        instruments: [],
        mixer: { buses: [] },
        panPositions: 33,
        ledsDimmed: options.ledsDimmed,
        autoSaveFlash: 'none' as const,
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

test('runtime dispatches hardware input through native dispatch', async () => {
  const seen: any[] = [];
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push(message);
      return { seq: 0, messages: [snapshotMessage()] };
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
      return { seq: 0, messages: [snapshotMessage()] };
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
      return { seq: 0, messages: [snapshotMessage()] };
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
      return { seq: 0, messages: [snapshotMessage()] };
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
          releaseFirst.push(() =>
            resolve({ seq: 0, messages: [snapshotMessage()] }),
          );
          return;
        }
        resolve({ seq: 0, messages: [snapshotMessage()] });
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

test('runtime applies native transient presentation fields', async () => {
  let transient = true;
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => ({
      seq: 0,
      messages: [
        snapshotMessage({
          eventDotLevel: transient ? 4 : 0,
          transportFlash: transient ? 'measure' : 'none',
        }),
      ],
    }),
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  const snapshot = runtime.getSnapshot();
  assert.equal(snapshot.frame.eventDotLevel, 4);
  assert.equal(snapshot.frame.transportFlash, 'measure');

  transient = false;
  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();
  assert.equal(runtime.getSnapshot().frame.eventDotLevel, 0);
});

test('runtime applies native ledsDimmed to desktop NeoKey LEDs', async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => ({
      seq: 0,
      messages: [snapshotMessage({ ledsDimmed: true })],
    }),
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(
    runtime.getSnapshot().neoKeyLeds.space,
    MAGENTA_COLOR.map((channel) => Math.floor((channel * 600 + 5000) / 10000)),
  );
});

test('display off does not dim desktop NeoKey LEDs', async () => {
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => ({
      seq: 0,
      messages: [snapshotMessage({ displayOff: true, ledsDimmed: false })],
    }),
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 2 });
  await waitMicrotask();

  assert.deepEqual(
    runtime.getSnapshot().neoKeyLeds.space,
    MAGENTA_COLOR.map((channel) => Math.floor((channel * 7500 + 5000) / 10000)),
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

test('a direct reply without a snapshot keeps queued async batches until they apply', async () => {
  let emitAsync: (
    seq: number,
    messages: RuntimeRunnerMessage[],
  ) => void = () => {};
  let release: () => void = () => {};
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: () =>
      new Promise((resolve) => {
        release = () => resolve({ seq: 5, messages: [] });
      }),
    asyncRuntimeBatchListener: (handler) => {
      emitAsync = handler;
    },
  });
  runtime.dispatch({ type: 'grid_press', x: 1, y: 1 });
  const queued = snapshotMessage();
  queued.snapshot.display.title = 'Queued';
  emitAsync(3, [queued]);
  release();
  await new Promise((resolve) => setTimeout(resolve, 200));

  assert.equal(runtime.getSnapshot().frame.display.title, 'Queued');
});
