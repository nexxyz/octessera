import type { LedCell } from "@cellsymphony/device-contracts";
import { clamp, mod } from "./coreUtils";
import { GRID_DOMAIN, PLATFORM_CAPS, clampPartIndex } from "./platformCaps";
import type { AuxTurnBinding, MenuNode, ParamModAxis, ParamModAxisSlots, ParamModSlotBinding, PlatformState, RuntimeConfig } from "./platformTypes";

const EMPTY: ParamModAxisSlots = { x: [null, null], y: [null, null] };

export function emptyParamModAxisSlots(): ParamModAxisSlots {
  return structuredClone(EMPTY);
}

export function normalizeParamMods(value: unknown): ParamModAxisSlots {
  const src = value as any;
  return {
    x: [normalizeSlot(src?.x?.[0]), normalizeSlot(src?.x?.[1])],
    y: [normalizeSlot(src?.y?.[0]), normalizeSlot(src?.y?.[1])]
  };
}

function normalizeSlot(value: unknown): ParamModSlotBinding | null {
  const src = value as any;
  if (!src || typeof src.key !== "string" || !src.key) return null;
  const kind = src.kind === "enum" || src.kind === "bool" ? src.kind : "number";
  return {
    key: src.key,
    label: typeof src.label === "string" ? src.label : undefined,
    kind,
    min: typeof src.min === "number" ? src.min : undefined,
    max: typeof src.max === "number" ? src.max : undefined,
    step: typeof src.step === "number" ? src.step : undefined,
    options: Array.isArray(src.options) ? src.options.map(String) : undefined,
    invert: src.invert === true
  };
}

export function paramBindingFromMenuNode(node: MenuNode | undefined | null): AuxTurnBinding | null {
  if (!node || (node.kind !== "number" && node.kind !== "enum" && node.kind !== "bool")) return null;
  const binding: AuxTurnBinding = { key: node.key, label: node.label, kind: node.kind };
  if (node.kind === "number") {
    binding.min = node.min;
    binding.max = node.max;
    binding.step = node.step;
  } else if (node.kind === "enum") {
    binding.options = node.options;
  }
  return binding;
}

export function mapParamModGridCell(x: number, y: number): Array<{ axis: ParamModAxis; slot: 0 | 1 }> {
  if (x === 0 && y === 0) return [{ axis: "x", slot: 0 }, { axis: "y", slot: 0 }];
  if (x === 1 && y === 1) return [{ axis: "x", slot: 1 }, { axis: "y", slot: 1 }];
  const targets: Array<{ axis: ParamModAxis; slot: 0 | 1 }> = [];
  if (y === 0 || y === 1) targets.push({ axis: "x", slot: y as 0 | 1 });
  if (x === 0 || x === 1) targets.push({ axis: "y", slot: x as 0 | 1 });
  return targets;
}

export function applyParamModMapping<TState>(state: PlatformState<TState>, binding: AuxTurnBinding, x: number, y: number): { state: PlatformState<TState>; message: string } | null {
  const targets = mapParamModGridCell(x, y);
  if (targets.length === 0) return null;
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const parts = Array.isArray((state.runtimeConfig as any).parts) ? [...((state.runtimeConfig as any).parts as any[])] : [];
  const part = parts[activePart];
  if (!part) return null;
  const paramMods = normalizeParamMods(part.paramMods);
  const source = paramMods[targets[0].axis][targets[0].slot];
  const nextMode = nextToggleMode(source, binding.key);
  for (const target of targets) {
    paramMods[target.axis][target.slot] = nextMode === "clear" ? null : { ...binding, invert: nextMode === "invert" };
  }
  parts[activePart] = { ...part, paramMods };
  const axisLabel = targets.length === 2 ? `X/Y Slot ${targets[0].slot + 1}` : `${targets[0].axis.toUpperCase()} Slot ${targets[0].slot + 1}`;
  const action = nextMode === "clear" ? "cleared" : nextMode === "invert" ? "inverted" : "mapped";
  return {
    state: { ...state, runtimeConfig: { ...(state.runtimeConfig as any), parts } as any },
    message: `${axisLabel}: ${binding.label ?? binding.key} ${action}`
  };
}

function nextToggleMode(current: ParamModSlotBinding | null, key: string): "regular" | "invert" | "clear" {
  if (!current || current.key !== key) return "regular";
  return current.invert ? "clear" : "invert";
}

export function paramModOverlayToLeds<TState>(state: PlatformState<TState>, highlighted: AuxTurnBinding | null, brightness: number): LedCell[] | null {
  if (!state.system.shiftHeld || !highlighted || state.system.touchMode !== "none") return null;
  const b = clamp(brightness, 0.1, 1);
  const out = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => ({ r: 0, g: 0, b: 0 }));
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const part = ((state.runtimeConfig as any).parts ?? [])[activePart] ?? {};
  const paramMods = normalizeParamMods(part.paramMods);
  const lane = scaleLed({ r: 18, g: 18, b: 24 }, b);
  for (let x = 0; x < PLATFORM_CAPS.gridWidth; x += 1) {
    setLed(out, x, 0, lane);
    setLed(out, x, 1, lane);
  }
  for (let y = 0; y < PLATFORM_CAPS.gridHeight; y += 1) {
    setLed(out, 0, y, lane);
    setLed(out, 1, y, lane);
  }
  paintAxisSlot(out, paramMods.x[0], highlighted.key, "x", 0, b);
  paintAxisSlot(out, paramMods.x[1], highlighted.key, "x", 1, b);
  paintAxisSlot(out, paramMods.y[0], highlighted.key, "y", 0, b);
  paintAxisSlot(out, paramMods.y[1], highlighted.key, "y", 1, b);
  setLed(out, 0, 0, scaleLed({ r: 255, g: 255, b: 255 }, b));
  setLed(out, 1, 1, scaleLed({ r: 255, g: 255, b: 255 }, b));
  return out;
}

function paintAxisSlot(out: LedCell[], slot: ParamModSlotBinding | null, highlightedKey: string, axis: ParamModAxis, slotIndex: 0 | 1, brightness: number): void {
  if (!slot) return;
  const full = slot.key === highlightedKey;
  const base = slot.invert ? { r: 255, g: 0, b: 90 } : { r: 0, g: 255, b: 120 };
  const color = scaleLed(base, full ? brightness : brightness * 0.35);
  if (axis === "x") {
    for (let x = 0; x < PLATFORM_CAPS.gridWidth; x += 1) setLed(out, x, slotIndex, color);
  } else {
    for (let y = 0; y < PLATFORM_CAPS.gridHeight; y += 1) setLed(out, slotIndex, y, color);
  }
}

function setLed(out: LedCell[], x: number, y: number, color: LedCell): void {
  out[GRID_DOMAIN.toDisplayIndex({ x, y })] = color;
}

function scaleLed(color: LedCell, brightness: number): LedCell {
  return { r: Math.round(color.r * brightness), g: Math.round(color.g * brightness), b: Math.round(color.b * brightness) };
}

export function normalizedAxis(index: number, size: number, gridOffset: number): number {
  const shifted = mod(index + gridOffset, size);
  return shifted / Math.max(1, size - 1);
}

export function scaledParamModValue(binding: ParamModSlotBinding, axis: ParamModAxis, intent: { x: number; y: number }): unknown {
  const size = axis === "x" ? PLATFORM_CAPS.gridWidth : PLATFORM_CAPS.gridHeight;
  const index = axis === "x" ? intent.x : intent.y;
  const source = binding.invert ? 1 - normalizedAxis(index, size, 0) : normalizedAxis(index, size, 0);
  if (binding.kind === "enum" && binding.options?.length) {
    const idx = clamp(Math.round(source * (binding.options.length - 1)), 0, binding.options.length - 1);
    return binding.options[idx];
  }
  if (binding.kind === "bool") return source >= 0.5;
  const min = binding.min ?? 0;
  const max = binding.max ?? 127;
  const step = binding.step ?? 1;
  const raw = min + source * (max - min);
  const stepped = step > 0 ? Math.round(raw / step) * step : raw;
  return clamp(Number(stepped.toFixed(6)), min, max);
}

export function paramModsForPart(cfg: RuntimeConfig, partIndex: number): ParamModAxisSlots {
  const part = ((cfg as any).parts ?? [])[partIndex] ?? {};
  return normalizeParamMods(part.paramMods);
}
