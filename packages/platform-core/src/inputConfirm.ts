import { clamp } from "./coreUtils";
import type { Deps } from "./inputModifier";
import type { PlatformState, PlatformEffect } from "./index";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { applyModifierState } from "./inputModifier";
import { OLED_TEXT_LINES } from "./platformTypes";
import { nowMs } from "./timing";

export function handleConfirmInput<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  behavior: BehaviorEngine<TState, unknown>,
  deps: Deps<TState>,
  nextState: PlatformState<TState>,
  events: MusicalEvent[],
  effects: PlatformEffect[]
): { state: PlatformState<TState>; events: MusicalEvent[]; effects: PlatformEffect[] } | null {
  const c = nextState.system.confirm!;
  const modifier = applyModifierState(nextState.system, input, pressed(input), nowMs());
  if (modifier.handled) {
    nextState.system = modifier.system;
    if (modifier.combinedPressed) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: true }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
    if (modifier.combinedReleased) nextState.behaviorState = behavior.onInput(nextState.behaviorState, { type: "button_combined_modifier", pressed: false }, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
    return { state: nextState, events, effects };
  }
  if (input.type === "encoder_turn" && deps.isMainEncoderInput(input.id)) {
    if (c.kind === "help_info" && c.action.kind === "help_info") {
      const contentSlots = Math.max(1, OLED_TEXT_LINES - 3);
      const maxScroll = Math.max(0, c.action.lines.length - contentSlots);
      nextState.system = { ...nextState.system, confirm: { ...c, scroll: clamp(c.scroll + input.delta, 0, maxScroll) } };
    } else {
      nextState.system = { ...nextState.system, confirm: { ...c, cursor: clamp(c.cursor + input.delta, 0, c.options.length - 1) } };
    }
  } else if (input.type === "encoder_press" && deps.isMainEncoderInput(input.id)) {
    const choice = c.options[c.cursor];
    if (c.kind === "aux_unbind" && c.action.kind === "aux_unbind") {
      if (choice !== "Cancel") {
        nextState = deps.applyAuxUnbindChoice(nextState, c.action.encoderId, choice);
        deps.autoSaveEffect(nextState, effects);
      }
    } else if (c.kind === "text_dirty_exit") {
      if (choice === "Save") nextState = deps.executeConfirmed(nextState, c.action, effects, behavior);
      else if (c.action.kind === "text_dirty_exit") {
        nextState = deps.writeAnyValue(nextState, c.action.key, c.action.original);
        nextState.system = { ...nextState.system, textEdit: null };
        nextState.menu = { ...nextState.menu, editing: false };
        if (c.action.backAfter) nextState.menu = deps.backMenu(nextState.menu);
      }
    } else if (choice === "Yes" || choice === "Confirm") {
      nextState = deps.executeConfirmed(nextState, c.action, effects, behavior);
    }
    nextState.system = { ...nextState.system, confirm: null };
  } else if (input.type === "button_a" && pressed(input)) {
    nextState.system = { ...nextState.system, confirm: null };
  }
  nextState.behaviorState = behavior.onInput(nextState.behaviorState, input, { bpm: nextState.transport.bpm, emit: (event) => events.push(event) });
  return { state: nextState, events, effects };
}

function pressed(i: any): boolean {
  return typeof i.pressed === "boolean" ? i.pressed : true;
}
