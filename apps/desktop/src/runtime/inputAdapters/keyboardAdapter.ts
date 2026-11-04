import type { DeviceInput } from '@octessera/device-contracts';
import type { InputAction } from '../types';

type KeyEventLike = Pick<KeyboardEvent, 'code' | 'repeat' | 'shiftKey'>;

export type KeyboardMapper = {
  keydown(event: KeyEventLike): InputAction[];
  keyup(event: KeyEventLike): InputAction[];
  releaseAll(): InputAction[];
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

type Edge = { press: InputAction; release: InputAction | null };

const MAIN_PRESS: Edge = {
  press: wrap({ type: 'encoder_press', id: 'main' }),
  release: null,
};
const SPACE = button('button_s');

const EDGE_KEYS = new Map<string, Edge>([
  ['Enter', MAIN_PRESS],
  ['NumpadEnter', MAIN_PRESS],
  ['Backspace', button('button_a')],
  ['Escape', button('button_a')],
  ['Space', SPACE],
  ['ShiftLeft', modifier('shift')],
  ['ShiftRight', modifier('shift')],
  ['ControlLeft', modifier('fn')],
  ['ControlRight', modifier('fn')],
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
  const held = new Map<string, InputAction | null>();
  let altLeft = false;

  const press = (code: string, edge: Edge): InputAction[] => {
    held.set(code, edge.release);
    return [edge.press];
  };

  return {
    keydown(event) {
      const { code } = event;
      if (code === 'AltLeft') {
        altLeft = true;
        return [];
      }
      const turn = turnAction(code);
      if (turn) return [turn];
      if (event.repeat || held.has(code)) return [];
      if (code === 'Space' && event.shiftKey)
        return press(code, {
          press: { type: 'emergency_brake' },
          release: SPACE.release,
        });
      const edge =
        EDGE_KEYS.get(code) ?? auxClick(code) ?? gridEdge(code, altLeft);
      return edge ? press(code, edge) : [];
    },
    keyup(event) {
      if (event.code === 'AltLeft') altLeft = false;
      if (!held.has(event.code)) return [];
      const release = held.get(event.code) ?? null;
      held.delete(event.code);
      return release ? [release] : [];
    },
    releaseAll() {
      const releases = [...held.values()].filter(
        (release): release is InputAction => release !== null,
      );
      held.clear();
      altLeft = false;
      return releases;
    },
  };
}

function turnAction(code: string): InputAction | null {
  const mainDelta = MAIN_TURNS.get(code);
  if (mainDelta)
    return wrap({ type: 'encoder_turn', delta: mainDelta, id: 'main' });
  const aux = auxKey(code);
  if (!aux || aux.slot === 1) return null;
  return wrap({ type: 'encoder_turn', delta: aux.slot - 1, id: aux.id });
}

function auxClick(code: string): Edge | null {
  const aux = auxKey(code);
  if (aux?.slot !== 1) return null;
  return { press: wrap({ type: 'encoder_press', id: aux.id }), release: null };
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
    press: wrap({ type: 'grid_press', ...tile }),
    release: wrap({ type: 'grid_release', ...tile }),
  };
}

function button(type: 'button_a' | 'button_s'): Edge {
  return {
    press: wrap({ type, pressed: true }),
    release: wrap({ type, pressed: false }),
  };
}

function modifier(type: 'shift' | 'fn'): Edge {
  return {
    press: { type, active: true },
    release: { type, active: false },
  };
}

function wrap(input: DeviceInput): InputAction {
  return { type: 'device_input', input };
}
