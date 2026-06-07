import type { PlatformEffect, PlatformState } from "./index";
import type { AuxPressBinding } from "./platformTypes";
import { compactSourcePathFromKey, fxTypeShort, locate } from "./menuView";
import { makeToast } from "./toast";

function auxPathFromKey<TState>(state: PlatformState<TState>, key: string): string | null {
  return compactSourcePathFromKey(state, key);
}

export function auxPathFromPress<TState>(state: PlatformState<TState>, press: any): string | null {
  if (!press) return null;
  if (press.kind === "behavior_action" || press.actionType) {
    const activePart = Number((state.runtimeConfig as any).activePartIndex ?? 0) + 1;
    return `L1>P${activePart}`;
  }
  if (press.kind === "menu_action") {
    if (press.action?.type === "sample_assign_enter") {
      return `L3>I${Number(press.action.instrumentSlot) + 1}>Sample`;
    }
    if (press.action?.type === "fx_assign_enter") {
      return `L4>FX>${fxTypeShort(press.action.config?.fxType ?? "none")}`;
    }
  }
  return null;
}

function auxInputPrefix(kind: "press" | "turn", encoderId: string): string {
  const index = encoderId.startsWith("aux") ? encoderId.slice(3) : encoderId;
  const lead = kind === "press" ? "S" : "T";
  return `${lead}${index}:`;
}

function auxToastPrefix<TState>(state: PlatformState<TState>, kind: "press" | "turn", encoderId: string, keyOrPress?: string | any): string {
  const index = encoderId.startsWith("aux") ? encoderId.slice(3) : encoderId;
  const lead = kind === "press" ? "S" : "T";
  const base = `${lead}${index}`;
  let path: string | null = null;
  if (typeof keyOrPress === "string") {
    path = auxPathFromKey(state, keyOrPress);
  } else if (keyOrPress) {
    path = auxPathFromPress(state, keyOrPress);
  }
  return path ? `${base} ${path}` : `${base}:`;
}

function setAuxToast<TState>(state: PlatformState<TState>, message: string): PlatformState<TState> {
  return {
    ...state,
    system: {
      ...state.system,
      toast: makeToast(message, { current: state.system.toast, extend: true })
    }
  };
}

export function applyAuxUnbindChoice<TState>(state: PlatformState<TState>, encoderId: string, choice: string): PlatformState<TState> {
  const binding = state.system.auxBindings[encoderId];
  if (!binding) return setAuxToast(state, "No binding");
  let nextBinding: any = binding;
  if (choice === "Both") nextBinding = null;
  else if (choice === "Click") nextBinding = binding.turn ? { turn: binding.turn, press: null } : null;
  else if (choice === "Turn") nextBinding = binding.press ? { turn: null, press: binding.press } : null;
  const nextState = {
    ...state,
    runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: nextBinding } } as any,
    system: {
      ...state.system,
      auxBindings: {
        ...state.system.auxBindings,
        [encoderId]: nextBinding
      }
    }
  };
  return setAuxToast(nextState, "Unbound");
}

export function setAuxTurnBinding<TState>(state: PlatformState<TState>, encoderId: string, turn: any): PlatformState<TState> {
  const existing = state.system.auxBindings[encoderId] ?? null;
  const nextBinding = turn || existing?.press ? { turn: turn ?? null, press: existing?.press ?? null } : null;
  return {
    ...state,
    runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: nextBinding } } as any,
    system: {
      ...state.system,
      auxBindings: {
        ...state.system.auxBindings,
        [encoderId]: nextBinding
      }
    }
  };
}

export function setAuxPressBinding<TState>(state: PlatformState<TState>, encoderId: string, press: AuxPressBinding | null): PlatformState<TState> {
  const existing = state.system.auxBindings[encoderId] ?? null;
  const nextBinding = press || existing?.turn ? { turn: existing?.turn ?? null, press: press ?? null } : null;
  return {
    ...state,
    runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: nextBinding } } as any,
    system: {
      ...state.system,
      auxBindings: {
        ...state.system.auxBindings,
        [encoderId]: nextBinding
      }
    }
  };
}

export function assignAuxEncoder<TState>(state: PlatformState<TState>, encoderId: string, _effects: PlatformEffect[], deps: any): PlatformState<TState> {
  const view = locate(deps.menuTree(state), state, state.menu);
  const selected = view.siblings[state.menu.cursor];
  const existing = state.system.auxBindings[encoderId];
  const openUnbindConfirm = (next: PlatformState<TState>): PlatformState<TState> => ({
    ...next,
    system: {
      ...next.system,
      confirm: {
        kind: "aux_unbind",
        action: { kind: "aux_unbind", encoderId },
        cursor: 0,
        options: ["Both", "Click", "Turn", "Cancel"],
        scroll: 0
      }
    }
  });

  if (!selected || selected.kind === "group" || selected.kind === "spacer" || selected.kind === "text") {
    if (!existing) return setAuxToast(state, `${auxInputPrefix("press", encoderId)} No binding`);
    return openUnbindConfirm(state);
  }

  if (selected.kind === "number" || selected.kind === "enum" || selected.kind === "bool") {
    const key = (selected as any).key as string;
    if (!key) return state;
    if (existing?.turn && existing.turn.key === key) return openUnbindConfirm(state);
    const turn: any = { key, label: (selected as any).label, kind: selected.kind };
    if (selected.kind === "number") {
      turn.min = (selected as any).min;
      turn.max = (selected as any).max;
      turn.step = (selected as any).step;
    } else if (selected.kind === "enum") {
      turn.options = (selected as any).options;
    }
    const boundPfx = auxToastPrefix(state, "press", encoderId, key);
    const next = setAuxToast(
      {
        ...state,
        runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: { turn, press: existing?.press ?? null } } } as any,
        system: { ...state.system, auxBindings: { ...state.system.auxBindings, [encoderId]: { turn, press: existing?.press ?? null } } }
      },
      `${boundPfx} Bound turn: ${(selected as any).label}`
    );
    deps.autoSaveEffect(next, _effects);
    return next;
  }

  if (selected.kind === "action") {
    const action = (selected as any).action as any;
    if (action.type === "behavior_action") {
      const nextPress: any = deps.isSpawnActionType(action.actionType)
        ? { kind: "behavior_action", actionType: action.actionType, routeKey: "trigger.life.spawn_now", label: "Spawn Now" }
        : { kind: "behavior_action", actionType: action.actionType, label: (selected as any).label };
      if (existing?.press?.kind === "behavior_action" && existing.press.actionType === nextPress.actionType && existing.press.routeKey === nextPress.routeKey) {
        return openUnbindConfirm(state);
      }
      const boundPfx = auxToastPrefix(state, "press", encoderId, nextPress);
      const next = setAuxToast(
        {
          ...state,
          runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: { turn: existing?.turn ?? null, press: nextPress } } } as any,
          system: {
            ...state.system,
            auxBindings: {
              ...state.system.auxBindings,
              [encoderId]: { turn: existing?.turn ?? null, press: nextPress }
            }
          }
        },
        `${boundPfx} Bound click: ${(selected as any).label}`
      );
      deps.autoSaveEffect(next, _effects);
      return next;
    }
    if (action.type === "sample_assign_enter" || action.type === "fx_assign_enter") {
      const nextPress: any = { kind: "menu_action", action, label: (selected as any).label };
      if (existing?.press?.kind === "menu_action" && existing.press.action?.type === action.type) {
        if (
          action.type !== "sample_assign_enter"
          || (existing.press.action.type === "sample_assign_enter" && existing.press.action.instrumentSlot === action.instrumentSlot)
        ) {
          return openUnbindConfirm(state);
        }
      }
      const boundPfx = auxToastPrefix(state, "press", encoderId, nextPress);
      const next = setAuxToast(
        {
          ...state,
          runtimeConfig: { ...(state.runtimeConfig as any), auxBindings: { ...((state.runtimeConfig as any).auxBindings ?? {}), [encoderId]: { turn: existing?.turn ?? null, press: nextPress } } } as any,
          system: {
            ...state.system,
            auxBindings: {
              ...state.system.auxBindings,
              [encoderId]: { turn: existing?.turn ?? null, press: nextPress }
            }
          }
        },
        `${boundPfx} Bound click: ${(selected as any).label}`
      );
      deps.autoSaveEffect(next, _effects);
      return next;
    }
    return state;
  }

  if (!existing) return setAuxToast(state, `${auxInputPrefix("press", encoderId)} No binding`);
  return openUnbindConfirm(state);
}
