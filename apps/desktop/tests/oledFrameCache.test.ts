import test from 'node:test';
import assert from 'node:assert/strict';
import {
  OLED_HEIGHT,
  OLED_WIDTH,
  createOledFrameRevision,
  type RuntimeOledFrameMessage,
} from '@octessera/device-contracts';
import {
  acceptOledFrameReference,
  createOledFrameCache,
  ingestOledFrame,
} from '../src/runtime/oledFrameCache';

const FRAME_BYTES = OLED_WIDTH * OLED_HEIGHT * 2;

function base64(bytes: Uint8Array): string {
  let value = '';
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    value += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(value);
}

function frame(
  revision: number,
  bytes = new Uint8Array(FRAME_BYTES),
): RuntimeOledFrameMessage {
  return {
    type: 'oled_frame',
    revision: createOledFrameRevision(revision),
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be',
    pixelsBase64: base64(bytes),
  };
}

test('desktop OLED cache validates idempotency, conflicts, and references', () => {
  const cache = createOledFrameCache();
  ingestOledFrame(cache, frame(1));
  assert.equal(cache.acceptedRevision, 0);
  assert.equal(cache.candidateRevision, 1);
  assert.equal(cache.acceptedPixels, null);
  assert.equal(acceptOledFrameReference(cache, 1)?.length, FRAME_BYTES);
  assert.equal(cache.acceptedRevision, 1);
  ingestOledFrame(cache, frame(1));
  assert.equal(cache.fault, null);

  const changed = new Uint8Array(FRAME_BYTES);
  changed[0] = 1;
  ingestOledFrame(cache, frame(1, changed));
  assert.equal(cache.fault, 'conflict');
  ingestOledFrame(cache, frame(1));
  assert.equal(acceptOledFrameReference(cache, 1), cache.acceptedPixels);
  assert.equal(cache.fault, 'conflict');
  ingestOledFrame(cache, frame(2, changed));
  assert.equal(cache.candidateRevision, 2);
  assert.equal(acceptOledFrameReference(cache, 2), cache.acceptedPixels);
  assert.equal(cache.acceptedRevision, 2);
  assert.equal(acceptOledFrameReference(cache, 3), cache.pixels);
  assert.equal(cache.fault, 'future');
  assert.equal(acceptOledFrameReference(cache, 1), cache.pixels);
  assert.equal(cache.fault, 'stale');
  assert.equal(acceptOledFrameReference(cache, undefined), cache.pixels);
  assert.equal(cache.fault, 'missing');

  ingestOledFrame(cache, {
    ...frame(1),
    revision: 0 as RuntimeOledFrameMessage['revision'],
  });
  assert.equal(cache.fault, 'malformed');
  assert.equal(cache.acceptedRevision, 2);
});

test('desktop OLED cache keeps candidate and accepted conflicts sticky', () => {
  const cache = createOledFrameCache();
  const first = new Uint8Array(FRAME_BYTES).fill(1);
  const second = new Uint8Array(FRAME_BYTES).fill(2);
  const third = new Uint8Array(FRAME_BYTES).fill(3);

  ingestOledFrame(cache, frame(1, first));
  ingestOledFrame(cache, frame(1, second));
  assert.equal(cache.candidateRevision, 1);
  assert.equal(cache.candidatePixels, null);
  assert.equal(cache.fault, 'conflict');

  ingestOledFrame(cache, frame(1, first));
  assert.equal(cache.candidatePixels, null);
  assert.equal(acceptOledFrameReference(cache, 1), null);
  assert.equal(cache.fault, 'conflict');

  ingestOledFrame(cache, frame(2, second));
  assert.equal(cache.acceptedRevision, 0);
  assert.equal(cache.acceptedPixels, null);
  assert.equal(cache.candidateRevision, 2);
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 2);
  assert.equal(cache.fault, null);

  ingestOledFrame(cache, frame(2, third));
  assert.equal(cache.fault, 'conflict');
  ingestOledFrame(cache, frame(2, second));
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 2);
  assert.equal(cache.fault, 'conflict');

  ingestOledFrame(cache, frame(3, third));
  ingestOledFrame(cache, frame(3, first));
  assert.equal(cache.fault, 'conflict');
  ingestOledFrame(cache, frame(3, third));
  assert.equal(cache.fault, 'conflict');
  assert.equal(acceptOledFrameReference(cache, 3)?.[0], 2);
  assert.equal(cache.fault, 'conflict');
  ingestOledFrame(cache, frame(4, first));
  assert.equal(acceptOledFrameReference(cache, 4)?.[0], 1);
  assert.equal(cache.fault, null);
});

test('desktop OLED cache rejects conflicting completion bytes', () => {
  const cache = createOledFrameCache();
  ingestOledFrame(cache, frame(1, new Uint8Array(FRAME_BYTES).fill(0x11)));
  assert.equal(acceptOledFrameReference(cache, 1)?.[0], 0x11);
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 0x11);

  ingestOledFrame(cache, frame(2, new Uint8Array(FRAME_BYTES).fill(0x22)));
  ingestOledFrame(cache, frame(2, new Uint8Array(FRAME_BYTES).fill(0x33)));

  assert.equal(cache.fault, 'conflict');
  assert.equal(cache.acceptedRevision, 1);
  assert.equal(cache.acceptedPixels?.[0], 0x11);
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 0x11);
  assert.equal(cache.acceptedRevision, 1);
});

test('desktop OLED cache retains the last valid frame for malformed input', () => {
  const cache = createOledFrameCache();
  ingestOledFrame(cache, frame(1));
  const pixels = cache.acceptedPixels;
  ingestOledFrame(cache, { ...frame(2), width: 127 });
  assert.equal(cache.fault, 'malformed');
  assert.equal(cache.acceptedRevision, 0);
  assert.equal(cache.acceptedPixels, pixels);
  ingestOledFrame(cache, {
    ...frame(2),
    pixelsBase64: base64(new Uint8Array(1)),
  });
  assert.equal(cache.fault, 'malformed');
  ingestOledFrame(cache, { ...frame(2), format: 'rgb565le' as 'rgb565be' });
  assert.equal(cache.fault, 'malformed');
  ingestOledFrame(cache, { ...frame(2), height: 127 });
  assert.equal(cache.fault, 'malformed');

  ingestOledFrame(cache, frame(2, new Uint8Array(FRAME_BYTES).fill(2)));
  assert.equal(cache.candidateRevision, 2);
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 2);
  assert.equal(cache.fault, null);

  const lastPixels = cache.pixels;
  ingestOledFrame(cache, frame(1, new Uint8Array(FRAME_BYTES).fill(1)));
  assert.equal(cache.acceptedRevision, 2);
  assert.equal(cache.acceptedPixels, lastPixels);
});

test('desktop OLED cache rejects malformed Base64 and retains zero-revision state', () => {
  const cache = createOledFrameCache();
  ingestOledFrame(cache, { ...frame(1), pixelsBase64: 'not-base64!' });
  assert.equal(cache.fault, 'malformed');
  assert.equal(cache.revision, 0);
  assert.equal(cache.pixels, null);
  assert.equal(acceptOledFrameReference(cache, undefined), null);
  assert.equal(cache.fault, 'missing');
});

test('desktop OLED cache recovers from reference faults on the next valid revision', () => {
  const cache = createOledFrameCache();
  ingestOledFrame(cache, frame(1, new Uint8Array(FRAME_BYTES).fill(1)));

  assert.equal(acceptOledFrameReference(cache, 2), null);
  assert.equal(cache.fault, 'future');
  ingestOledFrame(cache, frame(2, new Uint8Array(FRAME_BYTES).fill(2)));
  assert.equal(acceptOledFrameReference(cache, 2)?.[0], 2);
  assert.equal(cache.fault, null);

  assert.equal(acceptOledFrameReference(cache, 1), cache.acceptedPixels);
  assert.equal(cache.fault, 'stale');
  ingestOledFrame(cache, frame(3, new Uint8Array(FRAME_BYTES).fill(3)));
  assert.equal(acceptOledFrameReference(cache, 3)?.[0], 3);
  assert.equal(cache.fault, null);
  assert.equal(cache.acceptedRevision, 3);
});
