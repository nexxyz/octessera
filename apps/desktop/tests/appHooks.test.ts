import test from 'node:test';
import assert from 'node:assert/strict';
import type { DeviceInput } from '@octessera/device-contracts';
import { createKeyboardMapper } from '../src/runtime/inputAdapters/keyboardAdapter';
import { bindKeyboardInputs } from '../src/ui/appHooks';

const keyEvent = (type: string, code: string): Event =>
  Object.assign(new Event(type), {
    code,
    repeat: false,
    shiftKey: false,
    getModifierState: () => false,
  });

test('a key held across a keyboard rebind is released on key up', () => {
  const target = new EventTarget();
  const mapper = createKeyboardMapper();
  const dispatched: DeviceInput[] = [];
  const runtime = { dispatch: (input: DeviceInput) => dispatched.push(input) };
  const bind = () => bindKeyboardInputs(target, mapper, runtime, () => {});

  const unbind = bind();
  target.dispatchEvent(keyEvent('keydown', 'ShiftLeft'));
  target.dispatchEvent(keyEvent('keydown', 'KeyC'));
  unbind();
  bind();
  target.dispatchEvent(keyEvent('keyup', 'ShiftLeft'));
  target.dispatchEvent(keyEvent('keyup', 'KeyC'));

  assert.deepEqual(dispatched, [
    { type: 'button_shift', pressed: true },
    { type: 'grid_press', x: 0, y: 0 },
    { type: 'button_shift', pressed: false },
    { type: 'grid_release', x: 0, y: 0 },
  ]);
});
