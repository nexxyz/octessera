import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buttonInputForClick,
  modifierForButton,
  wheelTurnFromDelta,
} from '../src/ui/ControlsPanel';

test('encoder wheel preserves direction and acceleration magnitude', () => {
  assert.deepEqual(wheelTurnFromDelta(120), { delta: 1, magnitude: 120 });
  assert.deepEqual(wheelTurnFromDelta(-3), { delta: -1, magnitude: 3 });
  assert.equal(wheelTurnFromDelta(0), null);
});

test('clickable controls produce device actions, while modifiers do not click', () => {
  assert.deepEqual(buttonInputForClick('back'), { type: 'button_a' });
  assert.deepEqual(buttonInputForClick('space'), { type: 'button_s' });
  assert.equal(buttonInputForClick('shift'), null);
  assert.equal(buttonInputForClick('fn'), null);
});

test('only modifier keys produce modifier press and release targets', () => {
  assert.equal(modifierForButton('shift'), 'shift');
  assert.equal(modifierForButton('fn'), 'fn');
  assert.equal(modifierForButton('space'), null);
  assert.equal(modifierForButton('back'), null);
});
