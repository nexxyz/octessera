import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { PlatformEffect, PlatformState } from "./index";
import { PLATFORM_CAPS } from "./platformCaps";
import { DEFAULT_PAN_POS } from "./runtimeDefaults";
import { makeToast } from "./toast";

type ActionDeps<TState> = {
  writeValue: <TConfig extends object>(cfg: TConfig, key: string, value: unknown) => TConfig;
  extractConfigPayload: (state: PlatformState<TState>) => any;
  resolveBehavior: (activeId: string) => any;
};

export function handleMenuAction<TState>(state: PlatformState<TState>, action: any, effects: PlatformEffect[], deps: ActionDeps<TState>): PlatformState<TState> {
  const openConfirm = (kind: any, pending: any, options: string[] = ["No", "Yes"]): PlatformState<TState> => ({
    ...state,
    system: { ...state.system, confirm: { kind, action: pending, cursor: 0, options, scroll: 0 } }
  });
  const toast = (message: string): PlatformState<TState> => ({
    ...state,
    system: { ...state.system, toast: makeToast(message) }
  });

  if (action.type === "refresh_presets") {
    effects.push({ type: "store_list_presets" });
    return state;
  }
  if (action.type === "preset_save_current") {
    const name = state.system.currentPresetName;
    if (!name) return toast("No preset loaded");
    return openConfirm("overwrite_preset", { kind: "preset_save", name }, ["Cancel", "Confirm"]);
  }
  if (action.type === "preset_load") return openConfirm("load_preset", { kind: "preset_load", name: action.name });
  if (action.type === "preset_delete") return openConfirm("delete_preset", { kind: "preset_delete", name: action.name });
  if (action.type === "preset_save") {
    const name = state.system.draftName.trim();
    if (name.length === 0) return toast("Name required");
    if (state.system.presetNames.includes(name)) return openConfirm("overwrite_preset", { kind: "preset_save", name }, ["Cancel", "Confirm"]);
    effects.push({ type: "store_save_preset", name, payload: deps.extractConfigPayload(state) });
    return state;
  }
  if (action.type === "preset_rename_pick") {
    const picked = action.name;
    return { ...state, system: { ...state.system, selectedPreset: picked, draftName: picked, nameCursor: picked.length } };
  }
  if (action.type === "preset_rename_apply") {
    const from = state.system.selectedPreset;
    const to = state.system.draftName.trim();
    if (!from) return toast("Pick preset");
    if (to.length === 0) return toast("Name required");
    if (from === to) return toast("Same name");
    if (state.system.presetNames.includes(to)) return openConfirm("overwrite_preset", { kind: "preset_rename", from, to });
    return openConfirm("rename_preset", { kind: "preset_rename", from, to });
  }
  if (action.type === "default_save") return openConfirm("save_default", { kind: "default_save" });
  if (action.type === "default_load") return openConfirm("load_default", { kind: "default_load" });
  if (action.type === "factory_load") return openConfirm("load_factory", { kind: "factory_load" });
  if (action.type === "synth_preset_load") {
    return openConfirm("load_synth_preset", { kind: "synth_preset_load", slot: action.slot, presetId: action.presetId, presetLabel: action.presetLabel }, ["Cancel", "Confirm"]);
  }
  if (action.type === "sample_assign_enter") {
    return {
      ...state,
      system: {
        ...state.system,
        sampleAssign: { instrumentSlot: action.instrumentSlot, sampleSlot: action.sampleSlot },
        toast: makeToast(`Assign: Inst ${action.instrumentSlot + 1} / Slot ${action.sampleSlot + 1}`)
      }
    };
  }
  if (action.type === "sample_assign_exit") {
    return {
      ...state,
      system: { ...state.system, sampleAssign: null }
    };
  }
  if (action.type === "trigger_probability_assign_enter") {
    return {
      ...state,
      system: {
        ...state.system,
        triggerProbabilityAssign: { partIndex: action.partIndex },
        toast: makeToast(`Trig Prob: P${action.partIndex + 1}`)
      }
    };
  }
  if (action.type === "trigger_probability_assign_exit") {
    return {
      ...state,
      system: { ...state.system, triggerProbabilityAssign: null }
    };
  }
  if (action.type === "fx_assign_enter") {
    return {
      ...state,
      system: {
        ...state.system,
        fxAssignMode: { config: structuredClone(action.config) },
        toast: makeToast(`Map FX: ${action.config.fxType}`)
      }
    };
  }
  if (action.type === "fx_assign_exit") {
    return {
      ...state,
      system: { ...state.system, fxAssignMode: null }
    };
  }
  if (action.type === "sample_browse_open") {
    const dir = action.dir ?? "";
    effects.push({ type: "sample_list_request", instrumentSlot: action.instrumentSlot, sampleSlot: action.sampleSlot, dir });
    return {
      ...state,
      system: {
        ...state.system,
        sampleBrowser: { instrumentSlot: action.instrumentSlot, sampleSlot: action.sampleSlot, dir, entries: [] },
        toast: makeToast("Loading samples...")
      }
    };
  }
  if (action.type === "sample_browse_enter") {
    const browser = state.system.sampleBrowser;
    if (!browser) return state;
    effects.push({ type: "sample_list_request", instrumentSlot: browser.instrumentSlot, sampleSlot: browser.sampleSlot, dir: action.path });
    return { ...state, system: { ...state.system, sampleBrowser: { ...browser, dir: action.path, entries: [] } } };
  }
  if (action.type === "sample_browse_up") {
    const browser = state.system.sampleBrowser;
    if (!browser) return state;
    const parts = browser.dir.split("/").filter((p: string) => p.length > 0);
    const dir = parts.length > 0 ? parts.slice(0, -1).join("/") : "";
    effects.push({ type: "sample_list_request", instrumentSlot: browser.instrumentSlot, sampleSlot: browser.sampleSlot, dir });
    return { ...state, system: { ...state.system, sampleBrowser: { ...browser, dir, entries: [] } } };
  }
  if (action.type === "sample_pick") {
    const browser = state.system.sampleBrowser;
    if (!browser) return state;
    const key = `instruments.${browser.instrumentSlot}.sample.slots.${browser.sampleSlot}.path`;
    const nextCfg = deps.writeValue(state.runtimeConfig, key, action.path);
    const next = {
      ...state,
      runtimeConfig: nextCfg,
      system: {
        ...state.system,
        toast: makeToast(`Sample set: ${action.path.split("/").pop() ?? action.path}`)
      }
    };
    if (next.runtimeConfig.autoSaveDefault) {
      effects.push({ type: "store_save_default", payload: deps.extractConfigPayload(next), mode: "deferred" });
    }
    return next;
  }
  if (action.type === "midi_select_output") {
    const nextCfg = deps.writeValue(state.runtimeConfig, "midi.outId", action.id);
    effects.push({ type: "midi_select_output", id: action.id });
    return { ...state, runtimeConfig: nextCfg };
  }
  if (action.type === "midi_select_input") {
    const nextCfg = deps.writeValue(state.runtimeConfig, "midi.inId", action.id);
    effects.push({ type: "midi_select_input", id: action.id });
    return { ...state, runtimeConfig: nextCfg };
  }
  if (action.type === "midi_panic") return openConfirm("midi_panic", { kind: "midi_panic" });
  if (action.type === "instrument_clone") {
    const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? [...((state.runtimeConfig as any).instruments as any[])] : [];
    const src = instruments[action.slot];
    if (!src) return state;
    const targetIdx = instruments.findIndex((inst: any) => inst?.type === "none");
    if (targetIdx < 0) return toast("All slots in use");
    instruments[targetIdx] = { ...structuredClone(src), autoName: true, name: src.name ?? "synth", midi: { enabled: false, channel: targetIdx } };
    return { ...state, runtimeConfig: { ...state.runtimeConfig, instruments } as any };
  }
  if (action.type === "instrument_reset") {
    const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? [...((state.runtimeConfig as any).instruments as any[])] : [];
    if (!instruments[action.slot]) return state;
    instruments[action.slot] = { type: "none", autoName: true, name: "none", midi: { enabled: false, channel: action.slot }, synth: {}, sample: { baseVelocity: 100, velocityLevelsEnabled: false, velocityLevels: { high: 120, medium: 85, low: 45 }, selectedSlot: 0, slots: Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, () => ({ path: null })), tuneSemis: 0, amp: {}, ampEnv: {}, filter: {}, filterEnv: {}, assignments: [] }, midiEngine: { velocity: 100, durationMs: 120 }, mixer: { route: "direct", panPos: DEFAULT_PAN_POS, volume: 100 } };
    return { ...state, runtimeConfig: { ...state.runtimeConfig, instruments } as any };
  }
  if (action.type === "behavior_action") {
    const behavior = deps.resolveBehavior(action.behaviorId);
    const newState = behavior.onInput(state.behaviorState, { type: "behavior_action", actionType: action.actionType } as DeviceInput, {
      bpm: state.transport.bpm,
      emit: () => {}
    });
    return { ...state, behaviorState: newState };
  }
  if (action.type === "xy_set_target") {
    const activeIndex = (state.runtimeConfig as any).activePartIndex ?? 0;
    const key = `parts.${activeIndex}.xy.${action.axis}`;
    const label = action.binding?.label ?? "(none)";
    const nextState = { ...state, runtimeConfig: deps.writeValue(state.runtimeConfig, key, action.binding) };
    const menu = state.menu;
    const newStack = menu.stack.length >= 2 ? [menu.stack[0]] : [];
    const cursor = menu.stack.length >= 2 ? menu.stack[1] : 0;
    const finalState = {
      ...nextState,
      menu: { ...menu, stack: newStack, cursor },
      system: { ...nextState.system, toast: makeToast(`X/Y ${action.axis.toUpperCase()}: ${label}`) }
    };
    if (finalState.runtimeConfig.autoSaveDefault) {
      effects.push({ type: "store_save_default", payload: deps.extractConfigPayload(finalState), mode: "deferred" });
    }
    return finalState;
  }
  if (action.type === "noop") return state;
  if (action.type === "menu_back") {
    const menu = state.menu;
    if (menu.editing) return { ...state, menu: { ...menu, editing: false } };
    if (menu.stack.length === 0) return state;
    const parentCursor = menu.stack[menu.stack.length - 1];
    return { ...state, menu: { ...menu, stack: menu.stack.slice(0, -1), cursor: parentCursor } };
  }
  return state;
}
