import assert from 'node:assert/strict';
import test from 'node:test';
import { oledFaultCopy } from '../src/ui/RuntimeStatusToaster';
import type { OledFrameCacheFault } from '../src/runtime/oledFrameCache';
import {
  acceptedHarness,
  harness,
  oledFrame,
  pixelBytes,
  send,
  snapshot,
  wait,
} from './simulatorRuntimeOledHarness';

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
  await wait();

  assert.equal(result.runtime.getSnapshot().frame.display.title, 'Boot');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await wait(130);

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

test('split async OLED delivery publishes no transient fault during healthy recovery', async () => {
  const result = await acceptedHarness();
  await wait(130);
  const published: Array<{
    fault: string | null;
    available: boolean;
    firstPixel: number;
  }> = [];
  result.runtime.subscribe((current) => {
    published.push({
      fault: current.oledFrameFault,
      available: current.oledFrameAvailable,
      firstPixel: current.frame.oled.pixels[0]!,
    });
  });

  result.emitAsync(2, [snapshot(2, 'Async reference')]);
  result.emitAsync(3, [oledFrame(2, 0x2a)]);
  await wait(30);

  const current = result.runtime.getSnapshot();
  assert.ok(published.every(({ fault }) => fault === null));
  assert.equal(current.oledFrameFault, null);
  assert.equal(current.oledFrameAvailable, true);
  assert.ok(current.frame.oled.pixels.every((byte) => byte === 0x2a));
  assert.deepEqual(
    published
      .slice(-2)
      .map(({ available, firstPixel }) => ({ available, firstPixel })),
    [
      { available: true, firstPixel: 0x11 },
      { available: true, firstPixel: 0x2a },
    ],
  );
});

test('unresolved async OLED future becomes visible after the bounded turn and keeps last-good pixels', async () => {
  const result = await acceptedHarness();
  await wait(130);

  result.emitAsync(2, [snapshot(2, 'Unresolved future')]);
  assert.equal(result.runtime.getSnapshot().oledFrameFault, null);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await wait(30);

  const current = result.runtime.getSnapshot();
  assert.equal(current.oledFrameFault, 'future');
  assert.equal(current.oledFrameAvailable, true);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  result.emitAsync(3, [oledFrame(2, 0x2a)]);
  await wait();
  assert.equal(result.runtime.getSnapshot().oledFrameFault, 'future');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));
});

test('equal async references do not extend the original grace deadline', async () => {
  const result = await acceptedHarness();
  await wait(130);

  result.emitAsync(2, [snapshot(2, 'First reference')]);
  await wait(10);
  result.emitAsync(3, [snapshot(2, 'Repeated reference')]);
  assert.equal(result.runtime.getSnapshot().oledFrameFault, null);
  await wait(10);

  assert.equal(result.runtime.getSnapshot().oledFrameFault, 'future');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));
});

test('direct snapshot-before-frame keeps its typed fault when a later async frame arrives alone', async () => {
  const result = harness();

  await send(result, [snapshot(1, 'Direct reference')]);
  let current = result.runtime.getSnapshot();
  assert.equal(current.oledFrameFault, 'future');
  assert.equal(current.oledFrameAvailable, false);
  assert.ok(pixelBytes(result).every((byte) => byte === 0));

  await wait(140);
  result.emitAsync(2, [oledFrame(1, 0x45)]);
  await wait();
  current = result.runtime.getSnapshot();
  assert.equal(current.oledFrameFault, 'future');
  assert.equal(current.oledFrameAvailable, false);
  assert.ok(pixelBytes(result).every((byte) => byte === 0));
});

test('direct OLED candidate conflict cancels grace and preserves the last-good frame', async () => {
  const result = await acceptedHarness();
  await wait(130);
  result.emitAsync(2, [snapshot(2, 'Async reference')]);
  await send(result, [oledFrame(2, 0x22)]);
  result.emitAsync(3, [oledFrame(2, 0x33)]);
  await wait();

  let current = result.runtime.getSnapshot();
  assert.equal(current.oledFrameFault, 'conflict');
  assert.equal(current.oledFrameAvailable, true);
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));

  await wait(30);
  current = result.runtime.getSnapshot();
  assert.equal(current.oledFrameFault, 'conflict');
  assert.ok(pixelBytes(result).every((byte) => byte === 0x11));
});

test('superseded async OLED references cannot accept an older frame', async () => {
  const cases = [
    [1, 2, snapshot(null), 'missing'],
    [1, 2, snapshot(0), 'malformed'],
    [2, 3, snapshot(1), 'stale'],
    [1, 2, snapshot(3), 'future'],
  ] as const;

  for (const [baseRevision, pending, superseding, fault] of cases) {
    const result = await acceptedHarness(baseRevision);
    await wait(130);

    result.emitAsync(2, [snapshot(pending)]);
    await wait();
    result.emitAsync(3, [superseding]);
    await wait();
    result.emitAsync(4, [oledFrame(pending, 0x2a)]);
    await wait(30);

    const current = result.runtime.getSnapshot();
    assert.equal(current.oledFrameFault, fault);
    assert.equal(current.oledFrameAvailable, true);
    assert.ok(pixelBytes(result).every((byte) => byte === 0x11));
  }
});

test('async batches matching direct responses remain duplicate-suppressed', async () => {
  const result = await acceptedHarness();
  let matchingSnapshots = 0;
  result.runtime.subscribe((current) => {
    if (current.frame.display.title === 'Direct') matchingSnapshots += 1;
  });

  await send(result, [oledFrame(2, 0x22), snapshot(2, 'Direct')]);
  result.emitAsync(2, [oledFrame(2, 0x22), snapshot(2, 'Direct')]);
  await wait(130);

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
