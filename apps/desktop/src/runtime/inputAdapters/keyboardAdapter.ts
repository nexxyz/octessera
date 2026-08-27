import type { DeviceInput } from '@octessera/device-contracts';
import type { InputAction } from '../types';

export function mapKeyboardEventToInputAction(
  event: KeyboardEvent,
): InputAction | null {
  const modifierAction = mapKeyboardModifierAction(event);
  if (modifierAction !== null) return modifierAction;
  return mapKeyboardDeviceAction(event.key);
}

export function mapKeyboardKeyupToInputAction(
  event: KeyboardEvent,
): InputAction | null {
  if (event.key === 'Shift') return { type: 'shift', active: false };
  if (event.key === 'Control') return { type: 'fn', active: false };
  if (event.key === 'Backspace')
    return wrap({ type: 'button_a', pressed: false });
  if (event.key === 'Escape') return wrap({ type: 'button_a', pressed: false });
  if (event.key === ' ') return wrap({ type: 'button_s', pressed: false });
  return null;
}

export function shouldPreventKeyboardDefault(event: KeyboardEvent): boolean {
  return mapKeyboardEventToInputAction(event) !== null;
}

function wrap(input: DeviceInput): InputAction {
  return { type: 'device_input', input };
}

function mapKeyboardModifierAction(event: KeyboardEvent): InputAction | null {
  if (event.key === 'Shift') return { type: 'shift', active: true };
  if (event.key === 'Control') return { type: 'fn', active: true };
  if (event.key === ' ' && event.shiftKey) return { type: 'emergency_brake' };
  return null;
}

function mapKeyboardDeviceAction(key: string): InputAction | null {
  if (key === 'ArrowLeft' || key === 'ArrowUp')
    return wrap({ type: 'encoder_turn', delta: -1, id: 'main' });
  if (key === 'ArrowRight' || key === 'ArrowDown')
    return wrap({ type: 'encoder_turn', delta: 1, id: 'main' });
  if (key === 'Enter') return wrap({ type: 'encoder_press', id: 'main' });
  if (key === 'Backspace') return wrap({ type: 'button_a', pressed: true });
  if (key === 'Escape') return wrap({ type: 'button_a', pressed: true });
  if (key === ' ') return wrap({ type: 'button_s', pressed: true });
  return null;
}
