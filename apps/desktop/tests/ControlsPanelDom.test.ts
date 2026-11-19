import test from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import type { DeviceInput } from '@octessera/device-contracts';
import { installDom, fire, mount } from './domTestSupport';

const win = installDom();
const { ControlsPanel } = await import('../src/ui/ControlsPanel');
const { createSimulatorRuntime } =
  await import('../src/runtime/simulatorRuntime');

function mountControls() {
  const snapshot = createSimulatorRuntime(
    { start() {}, stop() {} },
    {
      runtimeDispatch: async () => ({ seq: 0, messages: [] }),
      audioLoadService: { listenAudioLoad: async () => () => {} },
    },
  ).getSnapshot();
  const dispatched: DeviceInput[] = [];
  const app = mount(
    createElement(ControlsPanel, {
      dialPhase: {},
      dispatch: (input: DeviceInput) => dispatched.push(input),
      frame: snapshot.frame,
      setDialDrag: () => {},
      setModifier: () => {},
      snapshot,
    }),
  );
  const dial = app.container.querySelector('.encoder-dial')!;
  return { app, dial, dispatched };
}

const wheel = (deltaY: number, deltaMode: number) =>
  new win.WheelEvent('wheel', {
    deltaY,
    deltaMode,
    bubbles: true,
    cancelable: true,
  });

test('one pixel-mode wheel notch on a dial dispatches exactly one encoder turn', () => {
  const { app, dial, dispatched } = mountControls();
  const event = wheel(100, 0);
  fire(dial, event);
  assert.deepEqual(dispatched, [
    { type: 'encoder_turn', delta: 1, id: 'main' },
  ]);
  assert.equal(event.defaultPrevented, true);
  app.unmount();
});

test('a line-mode wheel delta dispatches one encoder turn per line', () => {
  const { app, dial, dispatched } = mountControls();
  fire(dial, wheel(-3, 1));
  assert.deepEqual(dispatched, [
    { type: 'encoder_turn', delta: -1, id: 'main' },
    { type: 'encoder_turn', delta: -1, id: 'main' },
    { type: 'encoder_turn', delta: -1, id: 'main' },
  ]);
  app.unmount();
});
