import type { Deps } from "./inputModifier";
import type { PlatformEffect, PlatformState } from "./index";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { resolveAuxAutoMap } from "./auxAutoMap";

type AuxDeps<TState> = {
  menuTree: (state: PlatformState<TState>) => any;
  resolveBehavior: (id: string) => any;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  autoSaveEffect: (state: PlatformState<TState>, effects: PlatformEffect[]) => void;
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string;
  isSpawnActionType: (actionType: string) => boolean;
  spawnActionTypeForBehavior: (behaviorId: string) => string | null;
};

export function buildAuxDeps<TState>(deps: Deps<TState>): AuxDeps<TState> {
  return {
    menuTree: deps.menuTree,
    resolveBehavior: deps.resolveBehavior,
    readAnyValue: deps.readAnyValue,
    writeAnyValue: deps.writeAnyValue,
    reinitBehaviorState: deps.reinitBehaviorState,
    autoSaveEffect: deps.autoSaveEffect,
    formatDisplayValue: deps.formatDisplayValue,
    isSpawnActionType: deps.isSpawnActionType,
    spawnActionTypeForBehavior: deps.spawnActionTypeForBehavior
  };
}

export function handleAuxEncoderInput<TState>(
  state: PlatformState<TState>,
  input: DeviceInput,
  deps: Deps<TState>,
  auxDeps: AuxDeps<TState>,
  nextState: PlatformState<TState>,
  events: MusicalEvent[],
  effects: PlatformEffect[]
): { state: PlatformState<TState> } {
  if (input.type !== "encoder_press" && input.type !== "encoder_turn") return { state: nextState };
  if (deps.isMainEncoderInput(input.id)) return { state: nextState };

  const view = deps.locate(deps.menuTree(nextState), nextState, nextState.menu);
  const selected = view.siblings[nextState.menu.cursor] as any;
  const selectedKey = (selected && (selected.kind === "number" || selected.kind === "enum" || selected.kind === "bool")) ? String(selected.key ?? "") : undefined;
  const selectedAction = selected && selected.kind === "action" ? (selected.action as any) : null;
  const auto = resolveAuxAutoMap(nextState, { path: view.path, selectedKey, selectedAction }, deps.resolveBehavior);

  if (input.type === "encoder_press") {
    const slot = input.id === "aux1" ? auto.aux1 : input.id === "aux2" ? auto.aux2 : input.id === "aux3" ? auto.aux3 : auto.aux4;
    nextState = slot?.press
      ? deps.pressAuxEncoderMapped(nextState, input.id, slot.press, effects, (event: MusicalEvent) => events.push(event), auxDeps)
      : deps.pressAuxEncoder(nextState, input.id, effects, (event: MusicalEvent) => events.push(event), auxDeps);
  } else if (input.type === "encoder_turn") {
    const slot = input.id === "aux1" ? auto.aux1 : input.id === "aux2" ? auto.aux2 : input.id === "aux3" ? auto.aux3 : auto.aux4;
    nextState = slot?.turn
      ? deps.turnAuxEncoderMapped(nextState, input.id, slot.turn, input.delta, effects, auxDeps)
      : deps.turnAuxEncoder(nextState, input.id, input.delta, effects, auxDeps);
  }

  return { state: nextState };
}
