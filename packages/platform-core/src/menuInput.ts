import { clamp } from "./coreUtils";
import { locate } from "./menuView";
import type { MenuNode, PlatformEffect, PlatformState } from "./index";
import { clampInstrumentIndex, clampSampleSlotIndex } from "./platformCaps";
import { makeToast } from "./toast";
import { nowMs } from "./timing";

type PressDeps<TState> = {
  menuTree: (state: PlatformState<TState>) => MenuNode;
  handleAction: (state: PlatformState<TState>, action: any, effects: PlatformEffect[]) => PlatformState<TState>;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  formatTimestamp: (nowMs: number) => string;
  extractConfigPayload: (state: PlatformState<TState>) => any;
};

export function pressMenuInput<TState>(state: PlatformState<TState>, effects: PlatformEffect[], deps: PressDeps<TState>): PlatformState<TState> {
  const view = locate(deps.menuTree(state), state, state.menu);
  const selected = view.siblings[state.menu.cursor];
  if (!selected) return state;
  if (selected.kind === "spacer") return state;

  if (selected.kind === "group") {
    const label = selected.label ?? "";
    const partMatch = /^P(\d+):/.exec(label);
    if (partMatch) {
      const partIdx = parseInt(partMatch[1], 10) - 1;
      let nextState = deps.writeAnyValue(state, "activePartIndex", partIdx);
      nextState = deps.reinitBehaviorState(nextState, "activePartIndex");
      const nextMenu = { ...nextState.menu, stack: [...nextState.menu.stack, nextState.menu.cursor], cursor: 0 };
      return { ...nextState, menu: nextMenu };
    }
    const nextMenu = { ...state.menu, stack: [...state.menu.stack, state.menu.cursor], cursor: 0 };
    let nextState: PlatformState<TState> = { ...state, menu: nextMenu };
    if (label === "Presets" || label === "Load" || label === "Delete" || label === "Rename") effects.push({ type: "store_list_presets" });
    if (label === "MIDI Out") effects.push({ type: "midi_list_outputs_request" });
    if (label === "MIDI In") effects.push({ type: "midi_list_inputs_request" });
    if (label === "Save As") {
      const suggested = deps.formatTimestamp(nowMs());
      nextState = { ...nextState, system: { ...nextState.system, draftName: suggested, nameCursor: suggested.length } };
    }
    if (label === "Choose Sample") {
      const parts = view.path.split("/");
      const instrumentLabel = parts.find((p) => /^I\d+:/i.test(p.trim())) ?? "";
      const match = /^I(\d+):/i.exec(instrumentLabel.trim());
      if (match) {
        const instrumentSlot = clampInstrumentIndex(Number(match[1]) - 1);
        const selectedSlot = Number((nextState.runtimeConfig as any).instruments?.[instrumentSlot]?.sample?.selectedSlot ?? 0);
        const sampleSlot = clampSampleSlotIndex(selectedSlot);
        const browser = nextState.system.sampleBrowser;
        if (!browser || browser.instrumentSlot !== instrumentSlot || browser.sampleSlot !== sampleSlot) {
          const dir = browser && browser.instrumentSlot === instrumentSlot && browser.sampleSlot === sampleSlot ? browser.dir : "";
          effects.push({ type: "sample_list_request", instrumentSlot, sampleSlot, dir } as any);
          nextState = {
            ...nextState,
            system: {
              ...nextState.system,
              sampleBrowser: { instrumentSlot, sampleSlot, dir, entries: [] },
              toast: makeToast("Loading samples...")
            }
          };
        }
      }
    }
    return nextState;
  }

  if (selected.kind === "action") return deps.handleAction(state, selected.action, effects);
  if (selected.kind === "enum" && selected.key === "transport.playing") return { ...state, transport: { ...state.transport, playing: !state.transport.playing } };

  if (selected.kind === "text") {
    const current = String(deps.readAnyValue(state, selected.key) ?? "");
    if (!state.menu.editing) {
      return {
        ...state,
        menu: { ...state.menu, editing: true },
        system: {
          ...state.system,
          nameCursor: clamp(current.length, 0, selected.maxLen),
          textEdit: { key: selected.key, original: current, saveAction: selected.onExitSaveAction }
        }
      };
    }
    const nextCursor = clamp(state.system.nameCursor + 1, 0, selected.maxLen);
    return { ...state, system: { ...state.system, nameCursor: nextCursor } };
  }

  if (state.menu.editing && selected.kind === "bool" && selected.key === "autoSaveDefault" && state.runtimeConfig.autoSaveDefault) {
    effects.push({ type: "store_save_default", payload: deps.extractConfigPayload(state), mode: "immediate" });
  }
  return { ...state, menu: { ...state.menu, editing: !state.menu.editing } };
}

type TurnDeps<TState> = {
  menuTree: (state: PlatformState<TState>) => MenuNode;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  writeAnyValue: (state: PlatformState<TState>, key: string, value: unknown) => PlatformState<TState>;
  reinitBehaviorState: (state: PlatformState<TState>, key: string) => PlatformState<TState>;
  autoSaveEffect: (state: PlatformState<TState>, effects: PlatformEffect[]) => void;
  textEditTurn: (state: PlatformState<TState>, node: Extract<MenuNode, { kind: "text" }>, delta: -1 | 1) => PlatformState<TState>;
};

export function turnMenuInput<TState>(state: PlatformState<TState>, delta: -1 | 1, effects: PlatformEffect[], deps: TurnDeps<TState>): PlatformState<TState> {
  const view = locate(deps.menuTree(state), state, state.menu);
  if (!state.menu.editing) {
    const siblings = view.siblings;
    const max = Math.max(0, siblings.length - 1);
    let cursor = state.menu.cursor;
    let attempts = 0;
    do {
      cursor = clamp(cursor + delta, 0, max);
      attempts += 1;
    } while (siblings[cursor] && siblings[cursor].kind === "spacer" && attempts < siblings.length);
    return { ...state, menu: { ...state.menu, cursor } };
  }

  const selected = view.siblings[state.menu.cursor];
  if (!selected || selected.kind === "group" || selected.kind === "spacer" || selected.kind === "action") return state;
  if (selected.kind === "text") return deps.textEditTurn(state, selected, delta);

  if (selected.kind === "number") {
    const current = deps.readAnyValue(state, selected.key);
    const rawBase = Number(current);
    const base = Number.isFinite(rawBase) ? rawBase : selected.min;
    const nextValue = clamp(base + delta * selected.step, selected.min, selected.max);
    const nextState = deps.writeAnyValue(state, selected.key, nextValue);
    if (selected.key.startsWith("behaviorConfig.") || selected.key.includes(".l1.behaviorConfig.")) {
      const finalState = deps.reinitBehaviorState(nextState, selected.key);
      deps.autoSaveEffect(finalState, effects);
      return finalState;
    }
    deps.autoSaveEffect(nextState, effects);
    return nextState;
  }

  if (selected.kind === "bool") {
    const nextState = deps.writeAnyValue(state, selected.key, delta > 0);
    if (selected.key !== "autoSaveDefault") deps.autoSaveEffect(nextState, effects);
    return nextState;
  }

  const current = deps.readAnyValue(state, selected.key);
  const idx = selected.options.indexOf(String(current));
  const nextIdx = clamp(idx + delta, 0, selected.options.length - 1);
  const raw = selected.options[nextIdx];
  if (selected.key === "transport.playing") return { ...state, transport: { ...state.transport, playing: raw === "true" } };
  if (selected.key === "activeBehavior" || selected.key === "activePartIndex" || selected.key.endsWith(".l1.behaviorId") || selected.key.includes(".l1.behaviorConfig.") || selected.key.startsWith("behaviorConfig.")) {
    const nextState = deps.writeAnyValue(state, selected.key, raw);
    const finalState = deps.reinitBehaviorState(nextState, selected.key);
    deps.autoSaveEffect(finalState, effects);
    return finalState;
  }
  const nextState = deps.writeAnyValue(state, selected.key, raw);
  deps.autoSaveEffect(nextState, effects);
  return nextState;
}
