import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createOledFrameRevision,
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
  type NativeRuntimeSnapshot,
  type RuntimeOledFrameMessage,
  type RuntimeRunnerMessage,
} from '@octessera/device-contracts';
import { oledFaultCopy } from '../src/ui/RuntimeStatusToaster';
import { createSimulatorRuntime } from '../src/runtime/simulatorRuntime';
import type { RuntimeScheduler } from '../src/runtime/runtimeScheduler';
import type { OledFrameCacheFault } from '../src/runtime/oledFrameCache';

const FRAME_BYTES = OLED_WIDTH * OLED_HEIGHT * 2;

class FakeScheduler implements RuntimeScheduler {
  start(): void {}
  stop(): void {}
}

type Harness = {
  runtime: ReturnType<typeof createSimulatorRuntime>;
  setResponse: (messages: RuntimeRunnerMessage[]) => void;
  emitAsync: (seq: number, messages: RuntimeRunnerMessage[]) => void;
  emitAudioLoad: (status: { ratio: number; voiceSteal: boolean }) => void;
};

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

function oledFrame(revision: number, fill: number): RuntimeOledFrameMessage {
  return {
    type: 'oled_frame',
    revision: createOledFrameRevision(revision),
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be',
    pixelsBase64: base64(new Uint8Array(FRAME_BYTES).fill(fill)),
  };
}

function snapshot(
  revision: number | null,
  title = 'Boot',
  transportPlaying = false,
  masterVolume = 73,
): RuntimeRunnerMessage {
  const snapshotFields = {
    display: { page: 'boot', title, lines: [], editing: false },
    leds: {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      rgb: new Array<number>(GRID_WIDTH * GRID_HEIGHT * 3).fill(0),
      active: new Array<boolean>(GRID_WIDTH * GRID_HEIGHT).fill(false),
    },
    transport: { playing: transportPlaying, bpm: 120, tick: 0, ppqnPulse: 0 },
    activeBehavior: 'life',
    gridInteraction: 'paint' as const,
    neoKeyLeds: {
      back: [221, 130, 205] as [number, number, number],
      space: [221, 130, 205] as [number, number, number],
      shift: [67, 68, 71] as [number, number, number],
      fn: [67, 68, 71] as [number, number, number],
    },
    eventDotOn: false,
    transportIcon: 'stop' as const,
    transportFlash: 'none' as const,
    settings: {
      displayBrightness: 75,
      buttonBrightness: 75,
      masterVolume,
      voiceStealingMode: 'auto-balanced' as const,
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
  };
  const withRevision =
    revision === null
      ? snapshotFields
      : {
          ...snapshotFields,
          oledFrameRevision:
            revision > 0
              ? createOledFrameRevision(revision)
              : (revision as unknown as NativeRuntimeSnapshot['oledFrameRevision']),
        };
  return {
    type: 'snapshot',
    snapshot: withRevision as NativeRuntimeSnapshot,
  };
}

function harness(): Harness {
  let response: RuntimeRunnerMessage[] = [];
  let emitAsync: Harness['emitAsync'] = () => {};
  let emitAudioLoad: Harness['emitAudioLoad'] = () => {};
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => response,
    asyncRuntimeBatchListener: (handler) => {
      emitAsync = handler;
    },
    audioLoadService: {
      listenAudioLoad: async (handler) => {
        emitAudioLoad = handler;
        return () => {};
      },
    },
  });
  return {
    runtime,
    setResponse: (messages) => {
      response = messages;
    },
    emitAsync: (seq, messages) => emitAsync(seq, messages),
    emitAudioLoad: (status) => emitAudioLoad(status),
  };
}

async function send(harness: Harness, messages: RuntimeRunnerMessage[]) {
  harness.setResponse(messages);
  harness.runtime.dispatch({ type: 'grid_press', x: 1, y: 1 });
  await new Promise((resolve) => setTimeout(resolve, 0));
}

function pixelBytes(harness: Harness): Uint8Array {
  return harness.runtime.getSnapshot().frame.oled.pixels;
}

async function acceptedHarness(revision = 1): Promise<Harness> {
  const result = harness();
  await send(result, [oledFrame(revision, 0x11), snapshot(revision)]);
  return result;
}

test('simulator bootstraps a black OLED and hides an unreferenced candidate', async () => {
  const result = harness();
  assert.equal(result.runtime.getSnapshot().oledFrameAvailable, false);
  assert.ok(pixelBytes(result).every((byte) => byte === 0));

  await send(result, [oledFrame(1, 0x11)]);
  assert.equal(result.runtime.getSnapshot().oledFrameFault, null);
  assert.equal(result.runtime.getSnapshot().oledFrameAvailable, false);
  assert.ok(pixelBytes(result).every((byte) => byte === 0));
});

test('simulator accepts exact matching pixels and requires frame-before-snapshot ordering', async () => {
  const result = harness();
  await send(result, [oledFrame(1, 0x23), snapshot(1)]);
  assert.equal(result.runtime.getSnapshot().oledFrameFault, null);
  assert.equal(result.runtime.getSnapshot().oledFrameAvailable, true);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x23));

  const reversed = harness();
  await send(reversed, [snapshot(1), oledFrame(1, 0x45)]);
  assert.equal(reversed.runtime.getSnapshot().oledFrameFault, 'future');
  assert.equal(reversed.runtime.getSnapshot().oledFrameAvailable, false);
  assert.ok(pixelBytes(reversed).every((byte) => byte === 0));
  await send(reversed, [snapshot(1)]);
  assert.equal(reversed.runtime.getSnapshot().oledFrameFault, null);
  assert.ok(pixelBytes(reversed).every((byte) => byte === 0x45));
});

test('suppressed async OLED batches replay in order with their semantic snapshots', async () => {
  const result = await acceptedHarness();
  const observed: Array<{ title: string; firstPixel: number }> = [];
  result.runtime.subscribe((current) => {
    observed.push({
      title: current.frame.display.title,
      firstPixel: current.frame.oled.pixels[0]!,
    });
  });

  result.emitAsync(2, [oledFrame(2, 0x22), snapshot(2, 'Async two')]);
  result.emitAsync(3, [oledFrame(3, 0x33), snapshot(3, 'Async three')]);
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(result.runtime.getSnapshot().frame.display.title, 'Boot');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await new Promise((resolve) => setTimeout(resolve, 130));

  const current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Async three');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x33));
  assert.equal(current.oledFrameFault, null);
  assert.deepEqual(
    observed.filter(({ title }) => title.startsWith('Async')),
    [
      { title: 'Async two', firstPixel: 0x22 },
      { title: 'Async three', firstPixel: 0x33 },
    ],
  );
});

test('async batches matching direct responses remain duplicate-suppressed', async () => {
  const result = await acceptedHarness();
  let matchingSnapshots = 0;
  result.runtime.subscribe((current) => {
    if (current.frame.display.title === 'Direct') matchingSnapshots += 1;
  });

  await send(result, [oledFrame(2, 0x22), snapshot(2, 'Direct')]);
  result.emitAsync(2, [oledFrame(2, 0x22), snapshot(2, 'Direct')]);
  await new Promise((resolve) => setTimeout(resolve, 130));

  assert.equal(matchingSnapshots, 1);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x22));
});

test('malformed, missing, future, stale, and conflicting OLED data retain the last accepted frame', async () => {
  const malformed = await acceptedHarness();
  const malformedFrame = oledFrame(2, 0x22);
  malformedFrame.pixelsBase64 = 'not-base64!';
  await send(malformed, [malformedFrame]);
  assert.equal(malformed.runtime.getSnapshot().oledFrameFault, 'malformed');
  assert.ok(pixelBytes(malformed).every((byte) => byte === 0x11));

  const missing = await acceptedHarness();
  await send(missing, [snapshot(null)]);
  assert.equal(missing.runtime.getSnapshot().oledFrameFault, 'missing');
  assert.ok(pixelBytes(missing).every((byte) => byte === 0x11));

  const future = await acceptedHarness();
  await send(future, [snapshot(3, 'Future', true, 43)]);
  assert.equal(future.runtime.getSnapshot().oledFrameFault, 'future');
  assert.equal(future.runtime.getSnapshot().frame.display.title, 'Future');
  assert.equal(future.runtime.getSnapshot().frame.transport.playing, true);
  assert.equal(future.runtime.getSnapshot().masterVolume, 43);
  assert.ok(pixelBytes(future).every((byte) => byte === 0x11));

  const stale = await acceptedHarness(2);
  await send(stale, [oledFrame(1, 0x33)]);
  assert.equal(stale.runtime.getSnapshot().oledFrameFault, 'stale');
  assert.ok(pixelBytes(stale).every((byte) => byte === 0x11));
  await send(stale, [snapshot(1, 'Stale', true, 44)]);
  assert.equal(stale.runtime.getSnapshot().frame.display.title, 'Stale');
  assert.equal(stale.runtime.getSnapshot().frame.transport.playing, true);
  assert.equal(stale.runtime.getSnapshot().masterVolume, 44);
  assert.ok(pixelBytes(stale).every((byte) => byte === 0x11));

  const conflict = await acceptedHarness();
  await send(conflict, [
    oledFrame(2, 0x22),
    oledFrame(2, 0x33),
    snapshot(2, 'Conflict', true, 45),
  ]);
  assert.equal(conflict.runtime.getSnapshot().oledFrameFault, 'conflict');
  assert.equal(conflict.runtime.getSnapshot().frame.display.title, 'Conflict');
  assert.equal(conflict.runtime.getSnapshot().frame.transport.playing, true);
  assert.equal(conflict.runtime.getSnapshot().masterVolume, 45);
  assert.ok(pixelBytes(conflict).every((byte) => byte === 0x11));

  const malformedSnapshot = await acceptedHarness();
  await send(malformedSnapshot, [snapshot(0)]);
  assert.equal(
    malformedSnapshot.runtime.getSnapshot().oledFrameFault,
    'malformed',
  );
  assert.ok(pixelBytes(malformedSnapshot).every((byte) => byte === 0x11));
});

test('OLED reference faults preserve semantic snapshot fields', async () => {
  const result = await acceptedHarness();

  await send(result, [snapshot(null, 'Missing', true, 41)]);
  let current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Missing');
  assert.equal(current.frame.transport.playing, true);
  assert.equal(current.masterVolume, 41);
  assert.equal(current.oledFrameFault, 'missing');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await send(result, [snapshot(0, 'Malformed', false, 42)]);
  current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Malformed');
  assert.equal(current.frame.transport.playing, false);
  assert.equal(current.masterVolume, 42);
  assert.equal(current.oledFrameFault, 'malformed');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await send(result, [oledFrame(2, 0x22), snapshot(2, 'Recovered', true)]);
  current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Recovered');
  assert.equal(current.frame.transport.playing, true);
  assert.equal(current.oledFrameFault, null);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x22));
});

test('OLED reference faults preserve semantic state before the first accepted frame', async () => {
  const result = harness();

  await send(result, [snapshot(null, 'Missing', true, 51)]);
  let current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Missing');
  assert.equal(current.frame.transport.playing, true);
  assert.equal(current.masterVolume, 51);
  assert.equal(current.oledFrameFault, 'missing');
  assert.ok(pixelBytes(result).every((byte) => byte === 0));

  await send(result, [snapshot(0, 'Malformed', false, 52)]);
  current = result.runtime.getSnapshot();
  assert.equal(current.frame.display.title, 'Malformed');
  assert.equal(current.frame.transport.playing, false);
  assert.equal(current.masterVolume, 52);
  assert.equal(current.oledFrameFault, 'malformed');
  assert.ok(pixelBytes(result).every((byte) => byte === 0));
});

test('valid later revisions recover the accepted OLED frame and fault state', async () => {
  const result = await acceptedHarness();
  await send(result, [snapshot(3)]);
  await send(result, [oledFrame(3, 0x56), snapshot(3)]);
  assert.equal(result.runtime.getSnapshot().oledFrameFault, null);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x56));
});

test('semantic and audio-load updates preserve native OLED pixels', async () => {
  const result = await acceptedHarness();
  await send(result, [snapshot(1, 'Changed')]);
  assert.equal(result.runtime.getSnapshot().frame.display.title, 'Changed');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  result.emitAudioLoad({ ratio: 0.9, voiceSteal: true });
  assert.equal(result.runtime.getSnapshot().audioLoad.voiceSteal, true);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));
});

test('OLED faults expose typed copy without changing native-only rendering', () => {
  const faults = [
    'malformed',
    'conflict',
    'missing',
    'future',
    'stale',
  ] as const satisfies readonly OledFrameCacheFault[];
  assert.equal(
    oledFaultCopy('malformed', false),
    'OLED frame unavailable; showing blank display.',
  );
  for (const fault of faults) {
    assert.match(oledFaultCopy(fault, true), /OLED frame/);
  }
});
