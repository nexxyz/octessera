import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { PlatformEffect, PlatformState } from "./index";
import type { SystemState } from "./platformTypes";

export function applyModifierState(system: SystemState, input: DeviceInput, down: boolean, now: number): { system: SystemState; combinedPressed: boolean; combinedReleased: boolean; handled: boolean } {
  if (input.type !== "button_shift" && input.type !== "button_fn") return { system, combinedPressed: false, combinedReleased: false, handled: false };
  const physicalShiftHeld = input.type === "button_shift" ? down : system.physicalShiftHeld;
  const physicalFnHeld = input.type === "button_fn" ? down : system.physicalFnHeld;
  const combinedModifierHeld = physicalShiftHeld && physicalFnHeld;
  const shiftHeld = combinedModifierHeld ? false : physicalShiftHeld;
  const fnHeld = combinedModifierHeld ? false : physicalFnHeld;
  return {
    system: {
      ...system,
      physicalShiftHeld,
      physicalFnHeld,
      shiftHeld,
      fnHeld,
      combinedModifierHeld,
      shiftHeldSinceMs: shiftHeld ? (system.shiftHeld ? system.shiftHeldSinceMs : now) : null
    },
    combinedPressed: combinedModifierHeld && !system.combinedModifierHeld,
    combinedReleased: !combinedModifierHeld && system.combinedModifierHeld,
    handled: true
  };
}

export function reinitBehaviorConfig<TState>(
  state: PlatformState<TState>,
  deps: { resolveBehavior: (id: string) => any }
): PlatformState<TState> {
  const behaviorId = String((state.runtimeConfig as any).parts?.[(state.runtimeConfig as any).activePartIndex ?? 0]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
  const b = deps.resolveBehavior(behaviorId);
  const part: any = (state.runtimeConfig as any).parts?.[(state.runtimeConfig as any).activePartIndex ?? 0];
  const ns = (part?.l1?.behaviorConfig ?? state.runtimeConfig.behaviorConfig?.[behaviorId]) as Record<string, unknown> | undefined;
  const cfg: any = {};
  if (b.configMenu) for (const item of b.configMenu(b.init({}))) { const val = ns?.[item.key]; if (val !== undefined) cfg[item.key] = val; }
  const nextState = { ...state, behaviorState: b.init(cfg) };
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  if (Array.isArray((nextState as any).partStates) && (nextState as any).partStates.length > activePart) {
    (nextState as any).partStates[activePart] = nextState.behaviorState;
  }
  return nextState;
}

import { clampPartIndex } from "./platformCaps";

export type Deps<TState> = {
  isMainEncoderInput: (id: "main" | "aux1" | "aux2" | "aux3" | "aux4" | undefined) => boolean;
  applyAuxUnbindChoice: (state: PlatformState<TState>, encoderId: string, choice: string) => PlatformState<TState>;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  backMenu: (menu: any) => any;
  applyExternalClockPulses: (state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, pulses: number) => { state: PlatformState<TState>; events: MusicalEvent[] };
  locate: (root: any, state: PlatformState<TState>, menu: any) => any;
  menuTree: (state: PlatformState<TState>) => any;
  resolveBehavior: (activeId: string) => BehaviorEngine<any, any>;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  openContextHelp: (state: PlatformState<TState>) => PlatformState<TState>;
  pressMenu: (state: PlatformState<TState>, effects: PlatformEffect[]) => PlatformState<TState>;
  turnMenu: (state: PlatformState<TState>, delta: -1 | 1, effects: PlatformEffect[]) => PlatformState<TState>;
  assignAuxEncoder: any;
  pressAuxEncoder: any;
  turnAuxEncoder: any;
  pressAuxEncoderMapped: any;
  turnAuxEncoderMapped: any;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  autoSaveEffect: (state: PlatformState<TState>, effects: PlatformEffect[]) => void;
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string;
  isSpawnActionType: (actionType: string) => boolean;
  spawnActionTypeForBehavior: (behaviorId: string) => string | null;
  executeConfirmed: (state: PlatformState<TState>, action: any, effects: PlatformEffect[], behavior: BehaviorEngine<TState, unknown>) => PlatformState<TState>;
};
