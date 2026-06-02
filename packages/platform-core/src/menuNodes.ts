import { readValue } from "./coreUtils";
import type { MenuNode, PlatformState } from "./index";
import { PLATFORM_CAPS } from "./platformCaps";

export function presetListNodes<TState>(state: PlatformState<TState>, mode: "load" | "delete"): MenuNode[] {
  const names = state.system.presetNames;
  if (names.length === 0) {
    return [{ kind: "action", label: "(none)", action: { type: "refresh_presets" } }];
  }
  return names.map((name) => ({
    kind: "action",
    label: name,
    action: mode === "load" ? { type: "preset_load", name } : { type: "preset_delete", name }
  }));
}

export function presetRenameNodes<TState>(state: PlatformState<TState>): MenuNode[] {
  const names = state.system.presetNames;
  const picked = state.system.selectedPreset;
  const out: MenuNode[] = [];
  if (!picked) {
    if (names.length === 0) return [{ kind: "action", label: "(none)", action: { type: "refresh_presets" } }];
    return names.map((name) => ({ kind: "action", label: name, action: { type: "preset_rename_pick", name } }));
  }
  out.push({ kind: "action", label: `From: ${picked}`, action: { type: "refresh_presets" } });
  out.push({ kind: "text", label: "New Name", key: "system.draftName", maxLen: 32, onExitSaveAction: { type: "preset_rename_apply" } });
  out.push({ kind: "action", label: "Apply", action: { type: "preset_rename_apply" } });
  return out;
}

export function midiOutputNodes<TState>(state: PlatformState<TState>): MenuNode[] {
  const out: MenuNode[] = [];
  out.push({ kind: "action", label: "Disconnect", action: { type: "midi_select_output", id: null } });
  for (const p of state.system.midiOutputs) {
    out.push({ kind: "action", label: p.name.slice(0, 20), action: { type: "midi_select_output", id: p.id } });
  }
  if (out.length === 1) out.push({ kind: "action", label: "(no outputs)", action: { type: "midi_select_output", id: null } });
  return out;
}

export function midiInputNodes<TState>(state: PlatformState<TState>): MenuNode[] {
  const out: MenuNode[] = [];
  out.push({ kind: "action", label: "Disconnect", action: { type: "midi_select_input", id: null } });
  for (const p of state.system.midiInputs) {
    out.push({ kind: "action", label: p.name.slice(0, 20), action: { type: "midi_select_input", id: p.id } });
  }
  if (out.length === 1) out.push({ kind: "action", label: "(no inputs)", action: { type: "midi_select_input", id: null } });
  return out;
}

export function axisGroup(label: string, prefix: string, _defaultStep: number): MenuNode {
  const offsetLimit = prefix.endsWith(".x") || prefix === "x" ? PLATFORM_CAPS.gridWidth - 1 : PLATFORM_CAPS.gridHeight - 1;
  return {
    kind: "group",
    label,
    children: [
      {
        kind: "group",
        label: "Pitch Steps",
        children: [
          { kind: "bool", label: "Enabled", key: `${prefix}.pitch.enabled` },
          { kind: "number", label: "Steps", key: `${prefix}.pitch.steps`, min: -16, max: 16, step: 1, visible: (c) => readValue(c, `${prefix}.pitch.enabled`) === true },
          { kind: "bool", label: "Restart Section", key: `${prefix}.pitch.restartEachSection`, visible: (c) => readValue(c, `${prefix}.pitch.enabled`) === true }
        ]
      },
      laneGroup("Velocity", `${prefix}.velocity`, offsetLimit),
      laneGroup("Filter Cutoff", `${prefix}.filterCutoff`, offsetLimit),
      laneGroup("Filter Resonance", `${prefix}.filterResonance`, offsetLimit)
    ]
  };
}

function laneGroup(label: string, prefix: string, offsetLimit: number): MenuNode {
  return {
    kind: "group",
    label,
    children: [
      { kind: "bool", label: "Enabled", key: `${prefix}.enabled` },
      { kind: "number", label: "From", key: `${prefix}.from`, min: 0, max: 127, step: 1, visible: (c) => readValue(c, `${prefix}.enabled`) === true },
      { kind: "number", label: "To", key: `${prefix}.to`, min: 0, max: 127, step: 1, visible: (c) => readValue(c, `${prefix}.enabled`) === true },
      { kind: "number", label: "Grid Offset", key: `${prefix}.gridOffset`, min: -offsetLimit, max: offsetLimit, step: 1, visible: (c) => readValue(c, `${prefix}.enabled`) === true },
      { kind: "enum", label: "Curve", key: `${prefix}.curve`, options: ["linear", "curve"], visible: (c) => readValue(c, `${prefix}.enabled`) === true }
    ]
  };
}

export function sampleBrowserNodes<TState>(state: PlatformState<TState>, instrumentSlot: number, sampleSlot: number): MenuNode[] {
  const browser = (state.system as any).sampleBrowser;
  if (!browser || browser.instrumentSlot !== instrumentSlot || browser.sampleSlot !== sampleSlot) {
    return [];
  }
  const nodes: MenuNode[] = [{ kind: "action", label: "..", action: { type: "sample_browse_up" } }];
  for (const entry of browser.entries as Array<{ name: string; path: string; isDir: boolean }>) {
    if (entry.isDir) {
      nodes.push({ kind: "action", label: `[${entry.name}]`, action: { type: "sample_browse_enter", path: entry.path } });
    } else {
      nodes.push({ kind: "action", label: entry.name, action: { type: "sample_pick", path: entry.path } });
    }
  }
  if (nodes.length === 1) {
    nodes.push({ kind: "action", label: "(empty)", action: { type: "sample_browse_open", instrumentSlot, sampleSlot, dir: browser.dir } });
  }
  return nodes;
}
