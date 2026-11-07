import type { DeviceInput } from '@octessera/device-contracts';

type KeyEventLike = Pick<
  KeyboardEvent,
  'code' | 'repeat' | 'shiftKey' | 'getModifierState'
>;

type KeyboardMapper = {
  keydown(event: KeyEventLike): DeviceInput[];
  keyup(event: KeyEventLike): DeviceInput[];
  releaseAll(): DeviceInput[];
};

const GRID_KEY_ROWS = [
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

const AUX_KEY_ROWS = [
  ['Digit1', 'Digit2', 'Digit3'],
  ['KeyQ', 'KeyW', 'KeyE'],
  ['KeyA', 'KeyS', 'KeyD'],
];

const MAIN_TURNS = new Map<string, -1 | 1>([
  ['ArrowLeft', -1],
  ['ArrowUp', -1],
  ['ArrowRight', 1],
  ['ArrowDown', 1],
]);

type Edge = { press: DeviceInput; release: DeviceInput | null };

const MAIN_PRESS: Edge = {
  press: { type: 'encoder_press', id: 'main' },
  release: null,
};

const EDGE_KEYS = new Map<string, Edge>([
  ['Enter', MAIN_PRESS],
  ['NumpadEnter', MAIN_PRESS],
  ['Backspace', button('button_a')],
  ['Escape', button('button_a')],
  ['Space', button('button_s')],
  ['ShiftLeft', button('button_shift')],
  ['ShiftRight', button('button_shift')],
  ['ControlLeft', button('button_fn')],
  ['ControlRight', button('button_fn')],
]);

export function gridKeyTile(
  code: string,
  altLeft: boolean,
): { x: number; y: number } | null {
  for (const [row, codes] of GRID_KEY_ROWS.entries()) {
    const x = codes.indexOf(code);
    if (x >= 0) return { x, y: altLeft ? row + GRID_KEY_ROWS.length : row };
  }
  return null;
}

export function shouldPreventKeyboardDefault(event: KeyEventLike): boolean {
  return (
    event.code === 'AltLeft' ||
    EDGE_KEYS.has(event.code) ||
    MAIN_TURNS.has(event.code) ||
    auxKey(event.code) !== null ||
    gridKeyTile(event.code, false) !== null
  );
}

export function createKeyboardMapper(): KeyboardMapper {
  const held = new Map<string, DeviceInput | null>();
  let altLeft = false;
  let controlLeftJustPressed = false;

  const press = (code: string, edge: Edge): DeviceInput[] => {
    held.set(code, edge.release);
    controlLeftJustPressed = code === 'ControlLeft';
    return [edge.press];
  };

  const release = (code: string): DeviceInput[] => {
    if (!held.has(code)) return [];
    const action = held.get(code) ?? null;
    held.delete(code);
    return action ? [action] : [];
  };

  const altGr = (event: KeyEventLike): DeviceInput[] | null => {
    const afterControlLeft = controlLeftJustPressed;
    controlLeftJustPressed = false;
    if (event.code !== 'AltRight') return null;
    return afterControlLeft && event.getModifierState('AltGraph')
      ? release('ControlLeft')
      : [];
  };

  const keydown = (event: KeyEventLike): DeviceInput[] => {
    const { code } = event;
    if (code === 'AltLeft') {
      altLeft = true;
      return [];
    }
    const turn = turnAction(code);
    if (turn) return [turn];
    if (event.repeat || held.has(code)) return [];
    const edge =
      EDGE_KEYS.get(code) ?? auxClick(code) ?? gridEdge(code, altLeft);
    return edge ? press(code, edge) : [];
  };

  return {
    keydown(event) {
      return altGr(event) ?? keydown(event);
    },
    keyup(event) {
      controlLeftJustPressed = false;
      if (event.code === 'AltLeft') altLeft = false;
      return release(event.code);
    },
    releaseAll() {
      const releases = [...held.values()].filter(
        (release): release is DeviceInput => release !== null,
      );
      held.clear();
      altLeft = false;
      return releases;
    },
  };
}

function turnAction(code: string): DeviceInput | null {
  const mainDelta = MAIN_TURNS.get(code);
  if (mainDelta) return { type: 'encoder_turn', delta: mainDelta, id: 'main' };
  const aux = auxKey(code);
  if (!aux || aux.slot === 1) return null;
  return { type: 'encoder_turn', delta: aux.slot - 1, id: aux.id };
}

function auxClick(code: string): Edge | null {
  const aux = auxKey(code);
  if (aux?.slot !== 1) return null;
  return { press: { type: 'encoder_press', id: aux.id }, release: null };
}

function auxKey(code: string): { id: `aux${number}`; slot: number } | null {
  for (const [index, codes] of AUX_KEY_ROWS.entries()) {
    const slot = codes.indexOf(code);
    if (slot >= 0) return { id: `aux${index + 1}`, slot };
  }
  return null;
}

function gridEdge(code: string, altLeft: boolean): Edge | null {
  const tile = gridKeyTile(code, altLeft);
  if (!tile) return null;
  return {
    press: { type: 'grid_press', ...tile },
    release: { type: 'grid_release', ...tile },
  };
}

function button(
  type: 'button_a' | 'button_s' | 'button_shift' | 'button_fn',
): Edge {
  return {
    press: { type, pressed: true },
    release: { type, pressed: false },
  };
}
