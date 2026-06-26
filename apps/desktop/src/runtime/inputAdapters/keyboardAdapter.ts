import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { InputAction } from "../types";

export function mapKeyboardEventToInputAction(event: KeyboardEvent): InputAction | null {
  const key = event.key;
  if (key === "Shift") return { type: "shift", active: true };
  if (key === "Control") return { type: "fn", active: true };
  if (key === " " && event.shiftKey) return { type: "emergency_brake" };
  if (key === "ArrowLeft") return wrap({ type: "encoder_turn", delta: -1, id: "main" });
  if (key === "ArrowRight") return wrap({ type: "encoder_turn", delta: 1, id: "main" });
  if (key === "ArrowUp") return wrap({ type: "encoder_turn", delta: -1, id: "main" });
  if (key === "ArrowDown") return wrap({ type: "encoder_turn", delta: 1, id: "main" });
  if (key === "Enter") return wrap({ type: "encoder_press", id: "main" });
  if (key === "Backspace") return wrap({ type: "button_a", pressed: true });
  if (key === "Escape") return wrap({ type: "button_a", pressed: true });
  if (key === " ") return wrap({ type: "button_s", pressed: true });
  return null;
}

export function mapKeyboardKeyupToInputAction(event: KeyboardEvent): InputAction | null {
  if (event.key === "Shift") return { type: "shift", active: false };
  if (event.key === "Control") return { type: "fn", active: false };
  if (event.key === "Backspace") return wrap({ type: "button_a", pressed: false });
  if (event.key === "Escape") return wrap({ type: "button_a", pressed: false });
  if (event.key === " ") return wrap({ type: "button_s", pressed: false });
  return null;
}

export function shouldPreventKeyboardDefault(event: KeyboardEvent): boolean {
  return mapKeyboardEventToInputAction(event) !== null;
}

function wrap(input: DeviceInput): InputAction {
  return { type: "device_input", input };
}
