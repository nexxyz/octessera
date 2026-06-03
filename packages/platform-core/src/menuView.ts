import { abbreviatePath, barNumberChars, formatMenuItemLines, getSectionColor, getSectionColorFromPath, shouldUseNumberBar } from "./menuPresentation";
import { clamp } from "./coreUtils";
import type { BarValue, ConfirmState, MenuNode, MenuState, NumericDisplayMode, PlatformState } from "./platformTypes";
import { resolveAuxAutoMap, resolveEffectiveAuxMap } from "./auxAutoMap";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { AUX_MAPPING_OVERLAY_DELAY_MS, heldForMs, nowMs } from "./timing";

export function fxTypeShort(fxType: string): string {
  if (fxType === "pitch_shift") return "Pitch";
  if (fxType === "filter_sweep") return "Filter";
  if (fxType === "stutter") return "Stut";
  if (fxType === "freeze") return "Freeze";
  return "FX";
}

export function compactSourcePathFromKey<TState>(state: PlatformState<TState>, key: string): string | null {
  if (key.startsWith("touchFx.selected.params.")) {
    const fxType = String((state.runtimeConfig as any).touchFx?.selected?.fxType ?? "none");
    return `L4>FX>${fxTypeShort(fxType)}`;
  }
  const inst = /^instruments\.(\d+)\.(synth|sample)\.(.+)$/.exec(key);
  if (inst) {
     const idx = Number(inst[1]) + 1;
     const kind = inst[2] === "synth" ? "Synth" : "Sampler";
    const rest = inst[3];
    if (rest.startsWith("filterEnv.")) return `L3>I${idx}>${kind}>FEnv`;
    if (rest.startsWith("ampEnv.")) return `L3>I${idx}>${kind}>AEnv`;
    if (rest.startsWith("filter.")) return `L3>I${idx}>${kind}>Filter`;
    if (rest.startsWith("amp.")) return `L3>I${idx}>${kind}>Amp`;
    if (rest.startsWith("osc1.")) return `L3>I${idx}>Synth>Osc1`;
    if (rest.startsWith("osc2.")) return `L3>I${idx}>Synth>Osc2`;
    return `L3>I${idx}>${kind}`;
  }
  const mix = /^instruments\.(\d+)\.mixer\./.exec(key);
  if (mix) return `L3>I${Number(mix[1]) + 1}>Mixer`;

  const part = /^parts\.(\d+)\.l1\./.exec(key);
  if (part) return `L1>P${Number(part[1]) + 1}`;
  if (key.startsWith("behaviorConfig.")) return "L1";

  const bus = /^mixer\.buses\.(\d+)\.(slot[12])\.(?:params\.)?/.exec(key);
  if (bus) {
    const b = Number(bus[1]);
    const slotKey = bus[2];
    const slotIdx = slotKey === "slot1" ? 1 : 2;
    const type = String((state.runtimeConfig as any).mixer?.buses?.[b]?.[slotKey]?.type ?? "none");
    const label = type === "none" ? "None" : type;
    return `L3>B${b + 1}>S${slotIdx}>${label}`;
  }
  const busPan = /^mixer\.buses\.(\d+)\.panPos$/.exec(key);
  if (busPan) return `L3>B${Number(busPan[1]) + 1}`;

  return null;
}

export function visibleChildren<TState>(node: MenuNode, state: PlatformState<TState>): MenuNode[] {
  if (node.kind !== "group") return [];
  const kids = typeof node.children === "function" ? node.children(state) : node.children;
  return kids.filter((n) => ("visible" in n && typeof (n as any).visible === "function" ? (n as any).visible(state.runtimeConfig) : true));
}

function deriveGroupKey(group: any, groupPath: string): string | undefined {
  const pathInstMatch = /I(\d+):/i.exec(groupPath);
  if (!pathInstMatch) return undefined;
  const instIdx = Number(pathInstMatch[1]) - 1;
  const label = String(group.label ?? "").toLowerCase();
  const segs = groupPath.split("/").map(s => s.toLowerCase());

   const inSample = segs.some(s => s.includes("sampler")) || label.includes("sampler");
  const inMixer = segs.some(s => s.includes("mixer")) || label.includes("mixer");

  if (inSample) return `instruments.${instIdx}.sample.selectedSlot`;
  if (inMixer) return `instruments.${instIdx}.mixer.volume`;

  if (label.includes("envelope")) {
    return segs.some(s => s.includes("filter"))
      ? `instruments.${instIdx}.synth.filterEnv.attackMs`
      : `instruments.${instIdx}.synth.ampEnv.attackMs`;
  }
  if (label.includes("filter")) return `instruments.${instIdx}.synth.filter.cutoffHz`;
  if (label.includes("osc")) {
    return label.includes("2")
      ? `instruments.${instIdx}.synth.osc2.waveform`
      : `instruments.${instIdx}.synth.osc1.waveform`;
  }
  if (label.includes("volume") || label.includes("amp")) return `instruments.${instIdx}.synth.amp.gainPct`;
  if (label.includes("synth")) return `instruments.${instIdx}.synth.filter.cutoffHz`;

  return `instruments.${instIdx}.synth.filter.cutoffHz`;
}

export function locate<TState>(root: MenuNode, state: PlatformState<TState>, menu: MenuState): { node: MenuNode; siblings: MenuNode[]; path: string } {
  let node = root;
  const labels: string[] = [];
  for (const idx of menu.stack) {
    const kids = visibleChildren(node, state);
    const next = kids[idx] ?? kids[0];
    if (!next || next.kind !== "group") break;
    labels.push(next.label);
    node = next;
  }
  return { node, siblings: visibleChildren(node, state), path: labels.join("/") || "Menu" };
}

type CurrentMenuViewDeps<TState> = {
  state: PlatformState<TState>;
  menuTree: (state: PlatformState<TState>) => MenuNode;
  resolveBehavior: (id: string) => BehaviorEngine<any, any>;
  fitOledText: (text: string) => string;
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown;
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string;
  oledTextLines: number;
};

function barFraction(val: number, min: number, max: number): number {
  const range = max - min || 1;
  return Math.max(0, Math.min(1, (val - min) / range));
}

export function currentMenuView<TState>(deps: CurrentMenuViewDeps<TState>): { path: string; lines: string[]; colors: number[]; barValues: (BarValue | null)[] } {
  const { state, menuTree, resolveBehavior, fitOledText, readAnyValue, formatDisplayValue, oledTextLines } = deps;
  if (state.system.confirm) {
    const view = confirmView(state.system.confirm, fitOledText, oledTextLines);
    return { ...view, colors: Array(view.lines.length).fill(0xffff), barValues: Array(view.lines.length).fill(null) };
  }
  const { menu } = state;
  const { siblings, path } = locate(menuTree(state), state, menu);
  const shortPath = abbreviatePath(path);
  if (!siblings.length) return { path: shortPath, lines: [], colors: [], barValues: [] };

  // Shift-hold overlay: show current effective aux mappings after a delay.
  const now = nowMs();
  if (state.system.shiftHeld && heldForMs(now, state.system.shiftHeldSinceMs, AUX_MAPPING_OVERLAY_DELAY_MS)) {
    const cursor = clamp(menu.cursor, 0, siblings.length - 1);
    const focused = siblings[cursor] as any;
    let selectedKey = focused && (focused.kind === "number" || focused.kind === "enum" || focused.kind === "bool") ? String(focused.key ?? "") : undefined;
    const selectedAction = focused && focused.kind === "action" ? (focused.action as any) : null;
    if (!selectedKey && !selectedAction && focused?.kind === "group") {
      selectedKey = deriveGroupKey(focused, path);
    }
    const eff = resolveEffectiveAuxMap(state, { path, selectedKey, selectedAction }, resolveBehavior);
    const slots: Array<[string, typeof eff.aux1]> = [["A1", eff.aux1], ["A2", eff.aux2], ["A3", eff.aux3], ["A4", eff.aux4]];
    const anyAuto = slots.some(([, s]) => s.sourceTurn === "auto" || s.sourcePress === "auto");
    const anyCustom = slots.some(([, s]) => s.sourceTurn === "custom" || s.sourcePress === "custom");
    const title = anyAuto ? "AUTO MAP" : anyCustom ? "CUSTOM MAP" : "AUX MAP";

    const body: string[] = [];
    for (const [name, s] of slots) {
      const turnLabel = s.turn?.label ?? (s.turn?.key ? String(s.turn.key).split(".").slice(-1)[0] : "");
      const pressLabel = s.press?.label ?? (s.press?.kind === "behavior_action" ? s.press.actionType : "");
      const turnPath = s.turn?.key ? compactSourcePathFromKey(state, String(s.turn.key)) : null;
      const pressPath = (() => {
        const p = s.press;
        if (!p) return null;
        if (p.kind === "menu_action") {
          if ((p.action as any)?.type === "sample_assign_enter") {
            const i = Number((p.action as any).instrumentSlot) + 1;
            return `L3>I${i}>Sample`;
          }
          if ((p.action as any)?.type === "fx_assign_enter") {
            const fxType = String((p.action as any).config?.fxType ?? "none");
            return `L4>FX>${fxTypeShort(fxType)}`;
          }
        }
        if (p.kind === "behavior_action") {
          const activePart = Number((state.runtimeConfig as any).activePartIndex ?? 0) + 1;
          return p.routeKey === "trigger.life.spawn_now" ? `L1>P${activePart}` : `L1>P${activePart}`;
        }
        return null;
      })();

      if (!turnLabel && !pressLabel) {
        body.push(`${name}: -`);
        continue;
      }
      if (turnLabel && pressLabel && turnPath && pressPath && turnPath === pressPath) {
        body.push(`${name}: ${turnPath}`);
        body.push(`  ${turnLabel}`);
        body.push(`  !${pressLabel}`);
        continue;
      }
      if (turnLabel) {
        body.push(`${name}: ${turnPath ?? "?"}`);
        body.push(`  ${turnLabel}`);
      }
      if (pressLabel) {
        body.push(`${name}! ${pressPath ?? "?"}`);
        body.push(`  !${pressLabel}`);
      }
    }

    const contentSlots = Math.max(1, oledTextLines - 2);
    const maxScroll = Math.max(0, body.length - contentSlots);
    const scroll = clamp(state.system.auxOverlayScroll ?? 0, 0, maxScroll);
    const lines = body.slice(scroll, scroll + contentSlots).map((l) => fitOledText(l));
    return { path: title, lines, colors: Array(lines.length).fill(0xffff), barValues: Array(lines.length).fill(null) };
  }

  const cursor = clamp(menu.cursor, 0, siblings.length - 1);
  const focused = siblings[cursor] as any;
  let selectedKey = focused && (focused.kind === "number" || focused.kind === "enum" || focused.kind === "bool") ? String(focused.key ?? "") : undefined;
  const selectedAction = focused && focused.kind === "action" ? (focused.action as any) : null;
  if (!selectedKey && !selectedAction && focused?.kind === "group") {
    selectedKey = deriveGroupKey(focused, path);
  }
  const auto = resolveAuxAutoMap(state, { path, selectedKey, selectedAction }, resolveBehavior);
  const bodyBudget = Math.max(1, oledTextLines - 2);
  let start = cursor;
  let end = cursor + 1;
  let rowCount = formatMenuItemLines(siblings[cursor], state, true, menu.editing, fitOledText, readAnyValue, formatDisplayValue).length;

  while (rowCount < bodyBudget && (start > 0 || end < siblings.length)) {
    let grew = false;
    if (start > 0) {
      const prevRows = formatMenuItemLines(siblings[start - 1], state, false, false, fitOledText, readAnyValue, formatDisplayValue).length;
      if (rowCount + prevRows <= bodyBudget || end >= siblings.length) {
        start -= 1;
        rowCount += prevRows;
        grew = true;
      }
    }
    if (rowCount >= bodyBudget) break;
    if (end < siblings.length) {
      const nextRows = formatMenuItemLines(siblings[end], state, false, false, fitOledText, readAnyValue, formatDisplayValue).length;
      if (rowCount + nextRows <= bodyBudget || start === 0) {
        end += 1;
        rowCount += nextRows;
        grew = true;
      }
    }
    if (!grew) break;
  }

  const lines: string[] = [];
  const colors: number[] = [];
  const barValues: (BarValue | null)[] = [];
  const sectionColor = getSectionColorFromPath(path);

  const autoTurnPrefixForKey = (key: string): string | null => {
    const slots: Array<[string, any]> = [["1", auto.aux1], ["2", auto.aux2], ["3", auto.aux3], ["4", auto.aux4]];
    for (const [n, s] of slots) {
      if (s?.turn?.key === key) return `${n}-`;
    }
    return null;
  };

  const autoPressPrefixForAction = (action: any): string | null => {
    const slots: Array<[string, any]> = [["1", auto.aux1], ["2", auto.aux2], ["3", auto.aux3], ["4", auto.aux4]];
    for (const [n, s] of slots) {
      const p = s?.press;
      if (!p) continue;
      if (p.kind === "behavior_action" && action?.type === "behavior_action" && p.actionType === action.actionType) return n;
      if (p.kind === "menu_action" && action && p.action?.type === action.type) {
        if (action.type === "sample_assign_enter") {
          if (p.action.instrumentSlot === action.instrumentSlot) return n;
        } else {
          return n;
        }
      }
    }
    return null;
  };

  for (let i = start; i < end; i += 1) {
    let item: any = siblings[i];
    if (item && typeof item === "object" && typeof item.label === "string") {
      if ((item.kind === "number" || item.kind === "enum" || item.kind === "bool") && typeof item.key === "string") {
        const p = autoTurnPrefixForKey(item.key);
        if (p) item = { ...item, label: `${p}${item.label}` };
      } else if (item.kind === "action") {
        const a = item.action as any;
        if (a?.type === "behavior_action" && typeof a.actionType === "string") {
          const p = autoPressPrefixForAction(a);
          if (p) item = { ...item, actionPrefix: p } as any;
        } else if (a?.type === "sample_assign_enter" || a?.type === "fx_assign_enter") {
          const p = autoPressPrefixForAction(a);
          if (p) item = { ...item, actionPrefix: p } as any;
        }
      }
    }
    const isSelected = i === cursor && menu.editing;
    const itemLines = formatMenuItemLines(item, state, i === cursor, isSelected, fitOledText, readAnyValue, formatDisplayValue);
    if (item.kind === "spacer") {
      lines.push(...itemLines);
      colors.push(...Array(itemLines.length).fill(0x0000));
      barValues.push(...Array(itemLines.length).fill(null));
      continue;
    }
    lines.push(...itemLines);
    let itemColor = sectionColor;
    if (path === "Menu" || path === "") {
      itemColor = getSectionColor(item.label);
    }
    colors.push(...Array(itemLines.length).fill(itemColor));

    if (item.kind === "number" && shouldUseNumberBar(item)) {
      const mode = (state.runtimeConfig as any).numericDisplayMode as NumericDisplayMode;
      if (mode !== "numbers") {
        const val = Number(readAnyValue(state, item.key));
        const frac = barFraction(val, item.min, item.max);
        const numChars = mode === "bar+numbers" ? barNumberChars(item.key, item.min, item.max, formatDisplayValue, state.runtimeConfig as any) : 0;
        if (itemLines.length > 1) {
          barValues.push(null); // label line
          barValues.push({ frac, numChars }); // value line
        } else {
          barValues.push(null);
        }
        continue;
      }
    }
    barValues.push(...Array(itemLines.length).fill(null));
  }
  return { path: shortPath, lines: lines.slice(0, bodyBudget), colors: colors.slice(0, bodyBudget), barValues: barValues.slice(0, bodyBudget) };
}

function confirmView(confirm: ConfirmState, fitOledText: (text: string) => string, oledTextLines: number): { path: string; lines: string[] } {
  const title = confirm.kind === "text_dirty_exit" ? "TEXT" : confirm.kind === "help_info" ? "HELP" : "CONFIRM";
  if (confirm.kind === "help_info" && confirm.action.kind === "help_info") {
    const contentSlots = Math.max(1, oledTextLines - 3);
    const start = clamp(confirm.scroll, 0, Math.max(0, confirm.action.lines.length - contentSlots));
    const body = confirm.action.lines.slice(start, start + contentSlots).map((line) => fitOledText(line));
    return { path: title, lines: [...body, "@@> Close"].slice(0, oledTextLines - 2) };
  }
  const details = confirmDetails(confirm);
  const lines = [fitOledText(details)];
  for (let i = 0; i < confirm.options.length; i += 1) {
    const prefix = confirm.cursor === i ? "@@> " : "  ";
    lines.push(`${prefix}${confirm.options[i]}`);
  }
  return { path: title, lines: lines.slice(0, oledTextLines - 2) };
}

function confirmDetails(confirm: ConfirmState): string {
  const a = confirm.action;
  if (a.kind === "preset_save") return `Save ${a.name}?`;
  if (a.kind === "preset_delete") return `Delete? ${a.name}`;
  if (a.kind === "preset_load") return `Load? ${a.name}`;
  if (a.kind === "preset_rename") return `Rename? ${a.from}`;
  if (a.kind === "default_save") return "Save default?";
  if (a.kind === "default_load") return "Load default?";
  if (a.kind === "factory_load") return "Load factory?";
  if (a.kind === "text_dirty_exit") return "Save changes?";
  if (a.kind === "midi_panic") return "MIDI panic?";
  if (a.kind === "aux_unbind") return "Unbind encoder?";
  if (a.kind === "help_info") return "Help";
  return "Confirm";
}
