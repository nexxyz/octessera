import type { ActionSpec, MenuNode, NumericDisplayMode, PlatformState } from "./index";

export const COLOR_LIFE = 0x8ED1;
export const COLOR_SENSE = 0x8D5C;
export const COLOR_VOICE = 0xC59B;
export const COLOR_SEPIA = 0xB50D;

export function abbreviatePath(path: string): string {
  const map: Record<string, string> = {
    Menu: "MENU",
    L1: "L1",
    L2: "L2",
    L3: "L3",
    System: "SYS"
  };
  if (!path || path === "Menu") return "MENU";
  return path
    .split("/")
    .map((part) => map[part] ?? part)
    .join("/");
}

export function getSectionColorFromPath(path: string): number {
  if (path.startsWith("L1") || path.includes("L1:")) return COLOR_LIFE;
  if (path.startsWith("L2") || path.includes("L2:")) return COLOR_SENSE;
  if (path.startsWith("L3") || path.includes("L3:")) return COLOR_VOICE;
  if (path.includes("System") || path.includes("SYS")) return COLOR_SEPIA;
  if (path.includes("Menu") || path.includes("MENU")) return COLOR_SEPIA;
  return 0xffff;
}

export function getSectionColor(nodeLabel: string): number {
  if (nodeLabel.startsWith("L1:") || nodeLabel === "L1: Life") return COLOR_LIFE;
  if (nodeLabel.startsWith("L2:") || nodeLabel === "L2: Sense") return COLOR_SENSE;
  if (nodeLabel.startsWith("L3:") || nodeLabel === "L3: Voice") return COLOR_VOICE;
  if (nodeLabel === "System") return COLOR_SEPIA;
  return 0xffff;
}

export function isSpawnActionType(actionType: string): boolean {
  return actionType === "spawnRandom"
    || actionType === "seedRandom"
    || actionType === "spawnAnt"
    || actionType === "addBall"
    || actionType === "spawnPulse"
    || actionType === "dropNow"
    || actionType === "seedCluster"
    || actionType === "spawnGlider";
}

export function spawnActionTypeForBehavior(behaviorId: string): string | null {
  if (behaviorId === "life") return "spawnRandom";
  if (behaviorId === "brain") return "seedRandom";
  if (behaviorId === "ant") return "spawnAnt";
  if (behaviorId === "bounce") return "addBall";
  if (behaviorId === "pulse") return "spawnPulse";
  if (behaviorId === "raindrops") return "dropNow";
  if (behaviorId === "dla") return "seedCluster";
  if (behaviorId === "glider") return "spawnGlider";
  return null;
}

export function formatActionMenuLabel(item: Extract<MenuNode, { kind: "action" }>): string {
  const prefix = String((item as any).actionPrefix ?? "");
  return prefix ? `!${prefix}-${item.label}` : `!${item.label}`;
}

function actionDetailLine<TState>(state: PlatformState<TState>, item: Extract<MenuNode, { kind: "action" }>): string | null {
  if (item.action.type === "preset_save_current") {
    return state.system.currentPresetName ?? "(none loaded)";
  }
  return null;
}

const REVERB_DECAY_RE = /^mixer\.buses\.\d+\.slot[12]\.params\.decay$/;
const BAR_KEY_RE = /(?:\.params\.|(?:^|\.)(?:masterVolume|transport\.bpm|screenSleepSeconds)$|(?:Pct|Percent|Ms|Hz|Db|Semis|Cents|semitones|cents)$|(?:^|\.)(?:panPos|volume|baseVelocity|velocity|high|medium|low|durationMs|gainPct|velocitySensitivityPct|levelPct|pulseWidthPct|detuneCents|cutoffHz|resonance|envAmountPct|keyTrackingPct|attackMs|decayMs|sustainPct|releaseMs|noteLengthMs|velocityScalePct|steps|from|to|gridOffset|randomCellsPerTick|randomTickInterval|spawnStep|seedInterval|randomSeedCells|fireThreshold|maxAnts|autoSpawnInterval|spawnInterval|maxBalls|lifespan|maxRadius|autoPulseInterval|autoDropInterval|splashRadius))$/;
const NO_BAR_KEY_RE = /(?:^|\.)(?:channel|selectedSlot|activePartIndex|startingNote|lowestNote|highestNote)$/;

export function isReverbDecayKey(key: string): boolean {
  return REVERB_DECAY_RE.test(key);
}

export function shouldUseNumberBar(item: Extract<MenuNode, { kind: "number" }>): boolean {
  if (item.displayStyle === "number") return false;
  if (item.displayStyle === "bar") return true;
  if (NO_BAR_KEY_RE.test(item.key)) return false;
  return BAR_KEY_RE.test(item.key);
}

export function barNumberText(
  key: string,
  value: unknown,
  min: number,
  max: number,
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string,
  runtimeConfig?: any
): string {
  const formatted = formatBarValue(key, value, min, max, formatDisplayValue, runtimeConfig);
  const width = barNumberChars(key, min, max, formatDisplayValue, runtimeConfig);
  return formatted.padStart(width);
}

export function barNumberChars(
  key: string,
  min: number,
  max: number,
  formatDisplayValue?: (key: string, value: unknown, runtimeConfig?: any) => string,
  runtimeConfig?: any
): number {
  if (!formatDisplayValue) {
    const mn = Math.round(min);
    const mx = Math.round(max);
    const digits = Math.max(String(mn).length, String(mx).length);
    return (mn < 0 || mx < 0) ? digits + 1 : digits;
  }
  return Math.max(
    formatBarValue(key, min, min, max, formatDisplayValue, runtimeConfig).length,
    formatBarValue(key, (min + max) / 2, min, max, formatDisplayValue, runtimeConfig).length,
    formatBarValue(key, max, min, max, formatDisplayValue, runtimeConfig).length
  );
}

function formatBarValue(
  key: string,
  value: unknown,
  min: number,
  max: number,
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string,
  runtimeConfig?: any
): string {
  if (isUnitlessNormalizedRange(key, min, max)) {
    const range = max - min || 1;
    return String(Math.max(0, Math.min(100, Math.round(((Number(value) - min) / range) * 100))));
  }
  return formatDisplayValue(key, value, runtimeConfig);
}

function isUnitlessNormalizedRange(key: string, min: number, max: number): boolean {
  if (/decay$/.test(key) && isReverbDecayKey(key)) return false;
  return min >= 0 && max <= 1;
}

export function formatMenuItemLines<TState>(
  item: MenuNode,
  state: PlatformState<TState>,
  selected: boolean,
  editing: boolean,
  fitText: (text: string) => string,
  readAnyValue: (state: PlatformState<TState>, key: string) => unknown,
  formatDisplayValue: (key: string, value: unknown, runtimeConfig?: any) => string
): string[] {
  if (item.kind === "spacer") return [""];
  const mark = selected ? "@@" : "";
  if (item.kind === "group") return [`${mark}> ${item.label}`];
  if (item.kind === "action") {
    if (selected) {
      const detail = actionDetailLine(state, item);
      if (detail) return [`${mark} ${formatActionMenuLabel(item)}`, `${mark}  ${fitText(detail)}`];
    }
    return [`${mark} ${formatActionMenuLabel(item)}`];
  }
  if (item.kind === "text") {
    const value = String(readAnyValue(state, item.key) ?? "");
    const display = value.length === 0 ? "(empty)" : value;
    if (selected) return [`${mark}  ${item.label}:`, `${mark} ${editing ? " *" : "  "}${fitText(display)}`];
    return [`  ${item.label}`];
  }
  if (item.kind === "number" && shouldUseNumberBar(item)) {
    const mode = (state.runtimeConfig as any).numericDisplayMode as NumericDisplayMode;
    if (mode !== "numbers") {
      const val = Number(readAnyValue(state, item.key));
      const showNumeric = mode === "bar+numbers";
      const valueText = showNumeric ? barNumberText(item.key, val, item.min, item.max, formatDisplayValue, state.runtimeConfig as any) : "";
      if (selected) return [`${mark}  ${item.label}:`, `${mark} ${editing ? " *" : "  "}${fitText(valueText)}`];
      return [`  ${item.label}`];
    }
  }
  const value = formatDisplayValue(item.key, readAnyValue(state, item.key), state.runtimeConfig as any);
  if (selected) return [`${mark}  ${item.label}:`, `${mark} ${editing ? " *" : "  "}${fitText(value)}`];
  return [`  ${item.label}`];
}
