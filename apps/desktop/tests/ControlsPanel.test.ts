import test from 'node:test';
import assert from 'node:assert/strict';
import {
  buttonInputForClick,
  modifierForButton,
  wheelTurns,
} from '../src/ui/ControlsPanel';

test('one wheel notch turns the encoder one step at once', () => {
  assert.deepEqual(wheelTurns(0, 100, 0), { turns: 1, pending: 0 });
  assert.deepEqual(wheelTurns(0, -100, 0), { turns: -1, pending: 0 });
  assert.deepEqual(wheelTurns(0, 300, 0), { turns: 3, pending: 0 });
  assert.deepEqual(wheelTurns(0, -3, 1), { turns: -3, pending: 0 });
  assert.deepEqual(wheelTurns(0, 1, 2), { turns: 1, pending: 0 });
  assert.deepEqual(wheelTurns(0, 0, 0), { turns: 0, pending: 0 });
});

test('small trackpad events add up to one step per hundred pixels', () => {
  let pending = 0;
  const turns: number[] = [];
  for (let i = 0; i < 25; i += 1) {
    const result = wheelTurns(pending, 10, 0);
    pending = result.pending;
    turns.push(result.turns);
  }
  assert.equal(
    turns.reduce((sum, turn) => sum + turn, 0),
    2,
  );
  assert.equal(pending, 50);
});

test('small events in the other direction start a fresh count', () => {
  assert.deepEqual(wheelTurns(90, -10, 0), { turns: 0, pending: -10 });
  assert.deepEqual(wheelTurns(-95, -10, 0), { turns: -1, pending: -5 });
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
