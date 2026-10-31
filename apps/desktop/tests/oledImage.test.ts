import assert from 'node:assert/strict';
import test from 'node:test';
import { OLED_HEIGHT, OLED_WIDTH } from '@octessera/device-contracts';
import { oledPanelCurrentFactor, toOledImage } from '../src/ui/oledImage';

const FRAME_BYTES = OLED_WIDTH * OLED_HEIGHT * 2;

class TestImageData {
  readonly data: Uint8ClampedArray;
  readonly width: number;
  readonly height: number;

  constructor(data: Uint8ClampedArray, width: number, height: number) {
    this.data = data;
    this.width = width;
    this.height = height;
  }
}

(globalThis as unknown as { ImageData: typeof ImageData }).ImageData =
  TestImageData as unknown as typeof ImageData;

function frame(
  pixels = new Uint8Array(FRAME_BYTES),
  overrides: Partial<{
    width: number;
    height: number;
    format: string;
  }> = {},
) {
  return {
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be' as const,
    pixels,
    ...overrides,
  };
}

test('OLED image accepts only the exact 128x128 RGB565BE framebuffer', () => {
  const image = toOledImage(frame());
  assert.ok(image);
  assert.equal(image.width, 128);
  assert.equal(image.height, 128);
  assert.equal(image.data.length, 128 * 128 * 4);
  assert.equal(image.data[3], 255);

  assert.equal(toOledImage(frame(new Uint8Array(FRAME_BYTES - 2))), null);
  assert.equal(toOledImage(frame(new Uint8Array(FRAME_BYTES + 2))), null);
  assert.equal(toOledImage(frame(undefined, { width: 127 })), null);
  assert.equal(toOledImage(frame(undefined, { height: 127 })), null);
  assert.equal(toOledImage(frame(undefined, { format: 'rgb565le' })), null);
});

test('OLED image converts representative big-endian RGB565 pixels', () => {
  const pixels = new Uint8Array(FRAME_BYTES);
  pixels.set([0x00, 0x00, 0xff, 0xff, 0xf8, 0x00, 0x07, 0xe0, 0x00, 0x1f]);
  const image = toOledImage(frame(pixels));
  assert.ok(image);
  assert.deepEqual(
    Array.from(image.data.slice(0, 20)),
    [
      0, 0, 0, 255, 255, 255, 255, 255, 255, 0, 0, 255, 0, 255, 0, 255, 0, 0,
      255, 255,
    ],
  );
});

test('OLED Bright maps to the panel current share of the master contrast step', () => {
  const steps: [number, number][] = [
    [0, 0],
    [10, 1],
    [15, 1],
    [20, 2],
    [50, 7],
    [75, 11],
    [90, 13],
    [95, 14],
    [100, 15],
    [255, 15],
  ];
  for (const [percent, step] of steps) {
    assert.equal(
      oledPanelCurrentFactor(percent),
      (step + 1) / 16,
      `${percent}%`,
    );
  }
});

test('OLED image dims full-intensity pixels by the panel current', () => {
  const pixels = new Uint8Array(FRAME_BYTES);
  pixels.set([0xff, 0xff, 0xf8, 0x00]);
  const image = toOledImage(frame(pixels), oledPanelCurrentFactor(75));
  assert.ok(image);
  assert.deepEqual(
    Array.from(image.data.slice(0, 8)),
    [191, 191, 191, 255, 191, 0, 0, 255],
  );
});
