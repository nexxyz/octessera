import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { PlatformState } from "./index";
import { clampPartIndex } from "./platformCaps";

export function reinitBehaviorState<TState>(
  state: PlatformState<TState>,
  key: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): PlatformState<TState> {
  const previousBehaviorId = state.activeBehavior;
  if (key === "activePartIndex") {
    const targetPartIndex = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
    const partBehaviorId = String((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
    const partStates = Array.isArray((state as any).partStates) ? ((state as any).partStates as any[]) : [];
    const partState = partStates[targetPartIndex];
    return {
      ...(state as any),
      activeBehavior: partBehaviorId,
      behaviorState: (partState ?? state.behaviorState) as TState
    } as PlatformState<TState>;
  }
  const parts = key.split(".");
  let behaviorId = parts[1] ?? state.runtimeConfig.activeBehavior;
  let ns = state.runtimeConfig.behaviorConfig?.[behaviorId] as Record<string, unknown> | undefined;
  let targetPartIndex = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  if (key.startsWith("parts.")) {
    targetPartIndex = clampPartIndex(parts[1] ?? 0);
    behaviorId = String((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorId ?? state.runtimeConfig.activeBehavior);
    ns = ((state.runtimeConfig as any).parts?.[targetPartIndex]?.l1?.behaviorConfig ?? {}) as Record<string, unknown>;
  }
  const behavior = resolveBehavior(behaviorId);
  const cfg: any = {};
  if (behavior.configMenu) {
    for (const item of behavior.configMenu(behavior.init({}))) {
      const val = ns?.[item.key];
      if (val !== undefined) cfg[item.key] = val;
    }
  }
  const next = { ...state } as any;
  next.behaviorState = behavior.init(cfg);
  next.activeBehavior = behaviorId;
  if (key.endsWith(".l1.behaviorId")) {
    next.runtimeConfig = {
      ...next.runtimeConfig,
      activeBehavior: behaviorId,
      behaviorConfig: {
        ...(next.runtimeConfig.behaviorConfig ?? {}),
        [behaviorId]: { ...((next.runtimeConfig.behaviorConfig ?? {})[behaviorId] ?? {}) }
      }
    };
    const part = next.runtimeConfig.parts?.[targetPartIndex];
    if (part) {
      const seeded = { ...((next.runtimeConfig.behaviorConfig ?? {})[behaviorId] ?? {}) };
      part.l1 = { ...part.l1, behaviorId, behaviorConfig: seeded };
    }
  }
  if (Array.isArray(next.partStates) && next.partStates.length > targetPartIndex) {
    next.partStates[targetPartIndex] = next.behaviorState;
  }
  if (key === "activeBehavior" || key.endsWith(".l1.behaviorId")) {
    if (key.endsWith(".l1.behaviorId")) {
      next.runtimeConfig = {
        ...next.runtimeConfig,
        parts: remapPartParamModsForBehavior(next.runtimeConfig.parts ?? [], targetPartIndex, behaviorId, resolveBehavior)
      };
    }
    next.system = {
      ...next.system,
      auxBindings: remapAuxBindingsForBehavior(next.system.auxBindings, previousBehaviorId, behaviorId, resolveBehavior)
    };
  }
  return next as PlatformState<TState>;
}

const BEHAVIOR_PARAM_ANALOGUES = [
  ["randomTickInterval", "seedInterval", "autoSpawnInterval", "spawnInterval", "autoPulseInterval", "autoDropInterval"],
  ["randomCellsPerTick", "randomSeedCells", "maxAnts", "maxBalls"]
];

function remapPartParamModsForBehavior(
  parts: any[],
  partIndex: number,
  toBehaviorId: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): any[] {
  if (!Array.isArray(parts) || !parts[partIndex]?.paramMods) return parts;
  const next = [...parts];
  const part = { ...next[partIndex] };
  const paramMods = structuredClone(part.paramMods);
  for (const axis of ["x", "y"] as const) {
    for (let slotIdx = 0; slotIdx < 2; slotIdx += 1) {
      const slot = paramMods?.[axis]?.[slotIdx];
      if (!slot || typeof slot.key !== "string") continue;
      const match = new RegExp(`^parts\\.${partIndex}\\.l1\\.behaviorConfig\\.([^.]+)$`).exec(slot.key);
      if (!match) continue;
      paramMods[axis][slotIdx] = remapBehaviorTurnBindingForBehavior(slot, toBehaviorId, resolveBehavior, partIndex);
    }
  }
  next[partIndex] = { ...part, paramMods };
  return next;
}

function behaviorParamAnalogue(paramKey: string, behaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>): any | null {
  const behavior = resolveBehavior(behaviorId);
  if (!behavior.configMenu) return null;
  const items = behavior.configMenu(behavior.init({}));
  const keys = new Set<string>();
  for (const group of BEHAVIOR_PARAM_ANALOGUES) {
    if (group.includes(paramKey)) for (const k of group) keys.add(k);
  }
  if (keys.size === 0) keys.add(paramKey);
  for (const item of items) {
    if (!keys.has(item.key)) continue;
    if (item.type === "number") return { key: item.key, label: item.label, kind: "number", min: item.min ?? 0, max: item.max ?? 127, step: item.step ?? 1 };
    if (item.type === "enum") return { key: item.key, label: item.label, kind: "enum", options: item.options ?? [] };
    if (item.type === "bool") return { key: item.key, label: item.label, kind: "bool" };
  }
  return null;
}

function remapBehaviorTurnBindingForBehavior(binding: any, toBehaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>, partIndex?: number): any {
  if (!binding?.key || typeof binding.key !== "string") return binding;
  const partMatch = /^parts\.(\d+)\.l1\.behaviorConfig\.([^.]+)$/.exec(binding.key);
  const rootMatch = /^behaviorConfig\.([^.]+)\.([^.]+)$/.exec(binding.key);
  const paramKey = partMatch?.[2] ?? rootMatch?.[2];
  if (!paramKey) return binding;
  const analogue = behaviorParamAnalogue(paramKey, toBehaviorId, resolveBehavior);
  if (!analogue) return binding;
  if (partMatch) {
    const idx = partIndex ?? Number(partMatch[1]);
    return { ...binding, ...analogue, key: `parts.${idx}.l1.behaviorConfig.${analogue.key}` };
  }
  return { ...binding, ...analogue, key: `behaviorConfig.${toBehaviorId}.${analogue.key}` };
}

function primaryBehaviorAction(behaviorId: string, resolveBehavior: (id: string) => BehaviorEngine<any, any>): { actionType: string; label: string } | null {
  const behavior = resolveBehavior(behaviorId);
  if (!behavior.configMenu) return null;
  const items = behavior.configMenu(behavior.init({}));
  for (const item of items) {
    if (item.type === "action") return { actionType: item.key, label: item.label };
  }
  return null;
}

function remapAuxBindingsForBehavior(
  bindings: Record<string, any>,
  fromBehaviorId: string,
  toBehaviorId: string,
  resolveBehavior: (id: string) => BehaviorEngine<any, any>
): Record<string, any> {
  if (fromBehaviorId === toBehaviorId) return bindings;
  const fromAction = primaryBehaviorAction(fromBehaviorId, resolveBehavior);
  const toAction = primaryBehaviorAction(toBehaviorId, resolveBehavior);

  const next: Record<string, any> = { ...bindings };
  for (const id of Object.keys(next)) {
    const binding = next[id];
    if (!binding) continue;
    let nextBinding = binding.turn ? { ...binding, turn: remapBehaviorTurnBindingForBehavior(binding.turn, toBehaviorId, resolveBehavior) } : binding;
    if (fromAction && nextBinding?.press?.kind === "behavior_action" && !nextBinding.press.routeKey && nextBinding.press.actionType === fromAction.actionType) {
      if (!toAction) nextBinding = nextBinding.turn ? { turn: nextBinding.turn, press: null } : null;
      else nextBinding = { ...nextBinding, press: { kind: "behavior_action", actionType: toAction.actionType, label: toAction.label } };
    }
    next[id] = nextBinding;
  }
  return next;
}
