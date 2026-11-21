import test from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import type { DeviceInput } from '@octessera/device-contracts';
import { installDom, fire, mount, waitFor } from './domTestSupport';

const win = installDom();
const sentInputs: DeviceInput[] = [];
const tauriInternals = {
  transformCallback: () => 1,
  invoke: async (command: string, args?: Record<string, unknown>) => {
    if (command === 'runtime_drain_messages') return [];
    if (command === 'runtime_dispatch') {
      const message = args?.message as { type: string; input?: DeviceInput };
      if (message.type === 'device_input' && message.input)
        sentInputs.push(message.input);
      return { seq: 0, messages: [] };
    }
    return 1;
  },
};
Object.assign(win, {
  __TAURI_INTERNALS__: tauriInternals,
  __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener() {} },
});

const { App } = await import('../src/ui/App');

const key = (type: 'keydown' | 'keyup', code: string) =>
  new win.KeyboardEvent(type, { code, bubbles: true, cancelable: true });

const nonTurnInputs = () =>
  sentInputs.filter((input) => input.type !== 'encoder_turn');

test('the app shell mounts the OLED canvas and controls, then unmounts cleanly', async () => {
  sentInputs.length = 0;
  const app = mount(createElement(App));
  assert.ok(app.container.querySelector('canvas.oled-canvas'));
  assert.equal(app.container.querySelectorAll('.encoder-dial').length, 4);
  assert.equal(app.container.querySelectorAll('.encoder-center').length, 4);
  assert.ok(app.container.querySelector('.panel-layout'));

  app.unmount();
  assert.equal(app.container.childElementCount, 0);

  fire(win as unknown as EventTarget, key('keydown', 'ShiftLeft'));
  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.deepEqual(sentInputs, [], 'unmount removed the keyboard listeners');
});

test('held keys release exactly once after the app re-renders in between', async () => {
  sentInputs.length = 0;
  const app = mount(createElement(App));
  const window = win as unknown as EventTarget;

  fire(window, key('keydown', 'ShiftLeft'));
  fire(window, key('keydown', 'KeyC'));
  for (let i = 0; i < 3; i += 1) {
    fire(window, key('keydown', 'ArrowRight'));
    fire(window, key('keyup', 'ArrowRight'));
  }
  const ticks = () =>
    app.container.querySelectorAll('.encoder-tick.active').length;
  assert.ok(ticks() > 0, 'the dial re-rendered with a new phase');
  fire(window, key('keyup', 'KeyC'));
  fire(window, key('keyup', 'ShiftLeft'));

  await waitFor(() => nonTurnInputs().length >= 4);
  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.deepEqual(nonTurnInputs(), [
    { type: 'button_shift', pressed: true },
    { type: 'grid_press', x: 0, y: 0 },
    { type: 'grid_release', x: 0, y: 0 },
    { type: 'button_shift', pressed: false },
  ]);
  app.unmount();
});

test('a paint drag releases every tile it pressed', async () => {
  sentInputs.length = 0;
  const app = mount(createElement(App));
  const tile = (x: number, y: number) =>
    app.container.querySelector(`button[aria-label="Grid ${x},${y}"]`)!;

  fire(tile(0, 0), new win.MouseEvent('mousedown', { bubbles: true }));
  for (const x of [1, 2]) {
    fire(
      tile(x, 0),
      new win.MouseEvent('mouseover', { bubbles: true, buttons: 1 }),
    );
  }
  fire(tile(2, 0), new win.MouseEvent('mouseup', { bubbles: true }));

  await waitFor(() => nonTurnInputs().length >= 6);
  const columns = (type: string) =>
    nonTurnInputs()
      .filter((input) => input.type === type)
      .map((input) => (input as { x: number }).x)
      .sort();
  assert.deepEqual(columns('grid_press'), [0, 1, 2]);
  assert.deepEqual(columns('grid_release'), [0, 1, 2]);
  app.unmount();
});

test('losing window focus mid paint drag releases every tile it pressed', async () => {
  sentInputs.length = 0;
  const app = mount(createElement(App));
  const tile = (x: number, y: number) =>
    app.container.querySelector(`button[aria-label="Grid ${x},${y}"]`)!;

  fire(tile(0, 0), new win.MouseEvent('mousedown', { bubbles: true }));
  fire(
    tile(1, 0),
    new win.MouseEvent('mouseover', { bubbles: true, buttons: 1 }),
  );
  fire(win as unknown as EventTarget, new win.Event('blur'));

  await waitFor(() => nonTurnInputs().length >= 4);
  assert.deepEqual(
    nonTurnInputs()
      .filter((input) => input.type === 'grid_release')
      .map((input) => (input as { x: number }).x)
      .sort(),
    [0, 1],
  );
  app.unmount();
});
