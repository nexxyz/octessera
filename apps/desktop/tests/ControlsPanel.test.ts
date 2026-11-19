import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buttonInputForClick,
  modifierForButton,
  wheelTurnFromDelta,
} from '../src/ui/ControlsPanel';

test('one pixel-mode wheel notch turns the encoder one step', () => {
  assert.deepEqual(wheelTurnFromDelta(100, 0), { delta: 1, turns: 1 });
  assert.deepEqual(wheelTurnFromDelta(-100, 0), { delta: -1, turns: 1 });
  assert.deepEqual(wheelTurnFromDelta(-3, 0), { delta: -1, turns: 1 });
  assert.deepEqual(wheelTurnFromDelta(300, 0), { delta: 1, turns: 3 });
  assert.equal(wheelTurnFromDelta(0, 0), null);
});

test('line and page wheel deltas turn one step per unit', () => {
  assert.deepEqual(wheelTurnFromDelta(1, 1), { delta: 1, turns: 1 });
  assert.deepEqual(wheelTurnFromDelta(-3, 1), { delta: -1, turns: 3 });
  assert.deepEqual(wheelTurnFromDelta(1, 2), { delta: 1, turns: 1 });
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
