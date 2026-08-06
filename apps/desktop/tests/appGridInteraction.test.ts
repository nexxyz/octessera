import test from 'node:test';
import assert from 'node:assert/strict';
import {
  gridDisplayIndexFromDisplay,
  gridLogicalCellFromDisplay,
  momentaryCellTransition,
  paintCellTransition,
} from '../src/ui/appGridInteraction';

test('grid display coordinates map to lower-left logical coordinates', () => {
  assert.deepEqual(gridLogicalCellFromDisplay(0, 0), { x: 0, y: 7 });
  assert.deepEqual(gridLogicalCellFromDisplay(7, 7), { x: 7, y: 0 });
  assert.equal(gridDisplayIndexFromDisplay(2, 3), 26);
});

test('paint transition presses only when the cell changes and deduplicates drags', () => {
  const first = paintCellTransition({
    painted: new Set(),
    active: false,
    desired: true,
    x: 2,
    y: 3,
  });
  assert.deepEqual(first.input, { type: 'grid_press', x: 2, y: 4 });
  const repeated = paintCellTransition({
    painted: first.painted,
    active: false,
    desired: true,
    x: 2,
    y: 3,
  });
  assert.equal(repeated.input, null);
  const unchanged = paintCellTransition({
    painted: new Set(),
    active: true,
    desired: true,
    x: 2,
    y: 3,
  });
  assert.equal(unchanged.input, null);
});

test('paint transitions accumulate from the latest drag state', () => {
  let painted = new Set<string>();
  const inputs = [];
  for (const [x, y] of [
    [0, 0],
    [1, 0],
    [2, 0],
  ]) {
    const transition = paintCellTransition({
      painted,
      active: false,
      desired: true,
      x,
      y,
    });
    painted = transition.painted;
    if (transition.input) inputs.push(transition.input);
  }
  assert.deepEqual(inputs, [
    { type: 'grid_press', x: 0, y: 7 },
    { type: 'grid_press', x: 1, y: 7 },
    { type: 'grid_press', x: 2, y: 7 },
  ]);
  assert.equal(painted.size, 3);
});

test('momentary transition releases the previous cell before pressing the next', () => {
  assert.deepEqual(momentaryCellTransition(null, 0, 0).inputs, [
    { type: 'grid_press', x: 0, y: 7 },
  ]);
  assert.deepEqual(momentaryCellTransition({ x: 0, y: 7 }, 1, 0).inputs, [
    { type: 'grid_release', x: 0, y: 7 },
    { type: 'grid_press', x: 1, y: 7 },
  ]);
  assert.deepEqual(momentaryCellTransition({ x: 0, y: 7 }, 0, 0).inputs, []);
});
