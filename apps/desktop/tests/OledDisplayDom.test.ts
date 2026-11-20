import test from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import {
  OLED_HEIGHT,
  OLED_WIDTH,
  type RuntimeSnapshot,
} from '@octessera/device-contracts';
import { installDom, mount } from './domTestSupport';

const win = installDom();
const draws: number[] = [];
Object.assign(win.HTMLCanvasElement.prototype, {
  getContext: () => ({
    putImageData: () => draws.push(1),
    fillRect: () => draws.push(0),
  }),
});
const { OledDisplay } = await import('../src/ui/OledDisplay');

const frameWith = (pixels: Uint8Array) =>
  ({
    oled: {
      width: OLED_WIDTH,
      height: OLED_HEIGHT,
      format: 'rgb565be',
      pixels,
    },
  }) as unknown as RuntimeSnapshot;

const view = (pixels: Uint8Array) =>
  createElement(OledDisplay, {
    frame: frameWith(pixels),
    displayBrightness: 16,
  });

test('the OLED canvas keeps its native size and redraws only when the pixels change', () => {
  draws.length = 0;
  const pixels = new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2);
  const app = mount(view(pixels));
  const canvas = app.container.querySelector('canvas')!;
  assert.equal(canvas.width, OLED_WIDTH);
  assert.equal(canvas.height, OLED_HEIGHT);
  assert.deepEqual(draws, [1]);

  app.render(view(pixels));
  assert.deepEqual(draws, [1], 'a new snapshot with the same pixels');

  app.render(view(new Uint8Array(pixels)));
  assert.deepEqual(draws, [1, 1]);
  app.unmount();
});
