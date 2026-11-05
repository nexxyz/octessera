import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createKeyboardMapper,
  gridKeyTile,
  shouldPreventKeyboardDefault,
} from '../src/runtime/inputAdapters/keyboardAdapter';
import { momentaryTileTransition } from '../src/ui/appGridInteraction';

function keyEvent(
  code: string,
  { repeat = false, shiftKey = false, key = '', altGraph = false } = {},
): KeyboardEvent {
  const getModifierState = (name: string) => altGraph && name === 'AltGraph';
  return { code, key, repeat, shiftKey, getModifierState } as KeyboardEvent;
}

const device = (input: object) => ({ type: 'device_input', input });
const turn = (id: string, delta: number) =>
  device({ type: 'encoder_turn', delta, id });
const click = (id: string) => device({ type: 'encoder_press', id });
const tile = (type: string, x: number, y: number) => device({ type, x, y });

test('maps every key position by KeyboardEvent.code', () => {
  const expected: [string, object][] = [
    ['ArrowLeft', turn('main', -1)],
    ['ArrowUp', turn('main', -1)],
    ['ArrowRight', turn('main', 1)],
    ['ArrowDown', turn('main', 1)],
    ['Enter', click('main')],
    ['NumpadEnter', click('main')],
    ['Backspace', device({ type: 'button_a', pressed: true })],
    ['Escape', device({ type: 'button_a', pressed: true })],
    ['Space', device({ type: 'button_s', pressed: true })],
    ['ShiftLeft', { type: 'shift', active: true }],
    ['ShiftRight', { type: 'shift', active: true }],
    ['ControlLeft', { type: 'fn', active: true }],
    ['ControlRight', { type: 'fn', active: true }],
    ['Digit1', turn('aux1', -1)],
    ['Digit2', click('aux1')],
    ['Digit3', turn('aux1', 1)],
    ['KeyQ', turn('aux2', -1)],
    ['KeyW', click('aux2')],
    ['KeyE', turn('aux2', 1)],
    ['KeyA', turn('aux3', -1)],
    ['KeyS', click('aux3')],
    ['KeyD', turn('aux3', 1)],
  ];
  for (const [code, action] of expected) {
    assert.deepEqual(createKeyboardMapper().keydown(keyEvent(code)), [action]);
    assert.equal(shouldPreventKeyboardDefault(keyEvent(code)), true);
  }
});

test('grid keys map four keyboard rows to grid columns and rows', () => {
  const rows = [
    ['KeyC', 'KeyV', 'KeyB', 'KeyN', 'KeyM', 'Comma', 'Period', 'Slash'],
    ['KeyF', 'KeyG', 'KeyH', 'KeyJ', 'KeyK', 'KeyL', 'Semicolon', 'Quote'],
    ['KeyR', 'KeyT', 'KeyY', 'KeyU', 'KeyI', 'KeyO', 'KeyP', 'BracketLeft'],
    [
      'Digit4',
      'Digit5',
      'Digit6',
      'Digit7',
      'Digit8',
      'Digit9',
      'Digit0',
      'Minus',
    ],
  ];
  rows.forEach((codes, y) =>
    codes.forEach((code, x) => {
      assert.deepEqual(gridKeyTile(code, false), { x, y });
      assert.deepEqual(gridKeyTile(code, true), { x, y: y + 4 });
      assert.equal(shouldPreventKeyboardDefault(keyEvent(code)), true);
    }),
  );
});

test('unused keys stay unmapped and AltLeft only suppresses its default', () => {
  for (const code of ['KeyZ', 'KeyX', 'IntlBackslash', 'AltRight', 'F1']) {
    assert.deepEqual(createKeyboardMapper().keydown(keyEvent(code)), []);
    assert.equal(shouldPreventKeyboardDefault(keyEvent(code)), false);
  }
  assert.deepEqual(createKeyboardMapper().keydown(keyEvent('AltLeft')), []);
  assert.equal(shouldPreventKeyboardDefault(keyEvent('AltLeft')), true);
});

test('layout characters do not matter, only the key position', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('KeyQ', { key: 'a' })), [
    turn('aux2', -1),
  ]);
  assert.deepEqual(mapper.keydown(keyEvent('Semicolon', { key: 'm' })), [
    tile('grid_press', 6, 1),
  ]);
});

test('a keyboard grid press sends the same input as a simulator tile press', () => {
  const display = momentaryTileTransition(null, 0, 7).inputs;
  assert.deepEqual(createKeyboardMapper().keydown(keyEvent('KeyC')), [
    device(display[0]),
  ]);
  const topRight = momentaryTileTransition(null, 7, 0).inputs;
  const mapper = createKeyboardMapper();
  mapper.keydown(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keydown(keyEvent('Minus')), [device(topRight[0])]);
});

test('AltLeft selects the top half and AltRight is ignored', () => {
  const mapper = createKeyboardMapper();
  mapper.keydown(keyEvent('AltRight'));
  assert.deepEqual(mapper.keydown(keyEvent('KeyC')), [
    tile('grid_press', 0, 0),
  ]);
  mapper.keydown(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keydown(keyEvent('KeyV')), [
    tile('grid_press', 1, 4),
  ]);
  mapper.keyup(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keydown(keyEvent('KeyB')), [
    tile('grid_press', 2, 0),
  ]);
});

test('a held tile is released where it was pressed after Alt changes', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('KeyR')), [
    tile('grid_press', 0, 2),
  ]);
  mapper.keydown(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keyup(keyEvent('KeyR')), [
    tile('grid_release', 0, 2),
  ]);
  assert.deepEqual(mapper.keydown(keyEvent('KeyT')), [
    tile('grid_press', 1, 6),
  ]);
  mapper.keyup(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keyup(keyEvent('KeyT')), [
    tile('grid_release', 1, 6),
  ]);
  assert.deepEqual(mapper.keyup(keyEvent('KeyT')), []);
});

test('repeat is ignored for grid, clicks and buttons but accepted for turns', () => {
  const mapper = createKeyboardMapper();
  for (const code of ['KeyC', 'Digit2', 'Enter', 'NumpadEnter', 'Space']) {
    mapper.keydown(keyEvent(code));
    assert.deepEqual(mapper.keydown(keyEvent(code, { repeat: true })), []);
    assert.deepEqual(mapper.keydown(keyEvent(code)), []);
  }
  for (const [code, action] of [
    ['ArrowDown', turn('main', 1)],
    ['Digit3', turn('aux1', 1)],
    ['KeyA', turn('aux3', -1)],
  ] as const) {
    mapper.keydown(keyEvent(code));
    assert.deepEqual(mapper.keydown(keyEvent(code, { repeat: true })), [
      action,
    ]);
  }
});

test('modifier combos and releases keep their meaning', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('ShiftLeft', { shiftKey: true })), [
    { type: 'shift', active: true },
  ]);
  assert.deepEqual(mapper.keydown(keyEvent('Space', { shiftKey: true })), [
    { type: 'emergency_brake' },
  ]);
  assert.deepEqual(mapper.keyup(keyEvent('Space')), [
    device({ type: 'button_s', pressed: false }),
  ]);
  assert.deepEqual(mapper.keyup(keyEvent('ShiftLeft')), [
    { type: 'shift', active: false },
  ]);
  mapper.keydown(keyEvent('Escape'));
  assert.deepEqual(mapper.keyup(keyEvent('Escape')), [
    device({ type: 'button_a', pressed: false }),
  ]);
  mapper.keydown(keyEvent('Enter'));
  assert.deepEqual(mapper.keyup(keyEvent('Enter')), []);
});

test('focus loss releases every held tile, button and modifier once', () => {
  const mapper = createKeyboardMapper();
  for (const code of ['KeyC', 'Backspace', 'ControlLeft', 'KeyW']) {
    mapper.keydown(keyEvent(code));
  }
  mapper.keydown(keyEvent('AltLeft'));
  mapper.keydown(keyEvent('Slash'));
  assert.deepEqual(mapper.releaseAll(), [
    tile('grid_release', 0, 0),
    device({ type: 'button_a', pressed: false }),
    { type: 'fn', active: false },
    tile('grid_release', 7, 4),
  ]);
  assert.deepEqual(mapper.releaseAll(), []);
  assert.deepEqual(mapper.keydown(keyEvent('KeyC')), [
    tile('grid_press', 0, 0),
  ]);
});

const fnOn = { type: 'fn', active: true };
const fnOff = { type: 'fn', active: false };

test('AltGr takes back the fake left Control that Windows sends before it', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('ControlLeft')), [fnOn]);
  assert.deepEqual(
    mapper.keydown(keyEvent('AltRight', { altGraph: true, key: 'AltGraph' })),
    [fnOff],
  );
  assert.deepEqual(mapper.keydown(keyEvent('KeyC')), [
    tile('grid_press', 0, 0),
  ]);
  assert.deepEqual(
    mapper.keydown(keyEvent('ControlLeft', { repeat: true })),
    [],
  );
  assert.deepEqual(
    mapper.keydown(
      keyEvent('AltRight', { altGraph: true, key: 'AltGraph', repeat: true }),
    ),
    [],
  );
  assert.deepEqual(
    mapper.keyup(keyEvent('ControlLeft', { altGraph: true })),
    [],
  );
  assert.deepEqual(mapper.keyup(keyEvent('AltRight')), []);
  assert.deepEqual(mapper.releaseAll(), [tile('grid_release', 0, 0)]);
});

test('a real left Control is Fn at once and releases cleanly', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('ControlLeft')), [fnOn]);
  assert.deepEqual(mapper.keydown(keyEvent('KeyC')), [
    tile('grid_press', 0, 0),
  ]);
  assert.deepEqual(
    mapper.keydown(keyEvent('AltRight', { altGraph: true })),
    [],
  );
  assert.deepEqual(mapper.keyup(keyEvent('KeyC')), [
    tile('grid_release', 0, 0),
  ]);
  assert.deepEqual(mapper.keyup(keyEvent('ControlLeft')), [fnOff]);
  assert.deepEqual(mapper.releaseAll(), []);
});

test('right Alt without AltGraph keeps a real left Control held', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('ControlLeft')), [fnOn]);
  assert.deepEqual(mapper.keydown(keyEvent('AltRight')), []);
  assert.deepEqual(mapper.keyup(keyEvent('ControlLeft')), [fnOff]);
});

test('real left Control with left Alt is Fn on a top-half tile', () => {
  const mapper = createKeyboardMapper();
  assert.deepEqual(mapper.keydown(keyEvent('ControlLeft')), [fnOn]);
  assert.deepEqual(mapper.keydown(keyEvent('AltLeft')), []);
  assert.deepEqual(mapper.keydown(keyEvent('KeyC')), [
    tile('grid_press', 0, 4),
  ]);
  assert.deepEqual(mapper.keyup(keyEvent('KeyC')), [
    tile('grid_release', 0, 4),
  ]);
  mapper.keyup(keyEvent('AltLeft'));
  assert.deepEqual(mapper.keyup(keyEvent('ControlLeft')), [fnOff]);
});
