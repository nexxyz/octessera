import { PLATFORM_CAPS } from "./platformCaps";
import type { FxBusConfig, FxBusEffectType, PartConfig } from "./platformTypes";
import type { RuntimeConfig } from "./platformTypes";

const TRIGGER_KEYS = ["activate", "stable", "deactivate", "scanned", "scanned_empty"] as const;

export function mergeMapping(mapping: any, part: any, preferBase: boolean, slotField = "slot"): any {
  const next: any = { ...mapping };
  for (const key of TRIGGER_KEYS) {
    const m = mapping[key] ?? {};
    const p = part?.l2?.mapping?.[key] ?? {};
    next[key] = {
      ...m,
      action: preferBase ? (m.action ?? p.action) : (p.action ?? m.action),
      channel: Number(preferBase ? (m.channel ?? p[slotField] ?? 0) : (p[slotField] ?? m.channel ?? 0))
    };
  }
  return next;
}

export function overrideFromPart(mapping: any, part: any): any {
  const next: any = { ...mapping };
  for (const key of TRIGGER_KEYS) {
    const p = part?.l2?.mapping?.[key] ?? {};
    next[key] = { ...next[key], action: p.action, channel: Number(p.slot ?? next[key]?.channel ?? 0) };
  }
  return next;
}

export function preferMapping(mapping: any, part: any): any {
  const next: any = { ...mapping };
  for (const key of TRIGGER_KEYS) {
    const p = part?.l2?.mapping?.[key] ?? {};
    next[key] = { ...next[key], action: mapping[key]?.action ?? p.action, channel: Number(mapping[key]?.channel ?? p.slot ?? 0) };
  }
  return next;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

const CUTOFF_MIN_HZ = 80;
const CUTOFF_MAX_HZ = 16000;

export function cutoffDisplayToHz(display: number): number {
  const t = clamp(display, 0, 255) / 255;
  return Math.round(CUTOFF_MIN_HZ * Math.exp(t * Math.log(CUTOFF_MAX_HZ / CUTOFF_MIN_HZ)));
}

export function cutoffHzToDisplay(hz: number): number {
  const h = clamp(hz, CUTOFF_MIN_HZ, CUTOFF_MAX_HZ);
  return Math.round(Math.log(h / CUTOFF_MIN_HZ) / Math.log(CUTOFF_MAX_HZ / CUTOFF_MIN_HZ) * 255);
}

export function mod(value: number, base: number): number {
  return ((value % base) + base) % base;
}

export function fitOledText(text: string, columns: number): string {
  return fitOledTextToWidth(text, columns);
}

export function fitOledTextToWidth(text: string, width: number): string {
  if (text.length <= width) return text;
  if (width <= 3) return text.slice(0, width);
  return `${text.slice(0, width - 3)}...`;
}

export function fitOledMenuLine(line: string, columns: number): string {
  if (!line.startsWith("@@")) return fitOledText(line, columns);
  if (line.startsWith("@@> ")) {
    return `@@> ${fitOledTextToWidth(line.slice(4), columns - 2)}`;
  }
  return `@@${fitOledTextToWidth(line.slice(2), columns)}`;
}

export function wrapOledText(text: string, width: number): string[] {
  const normalized = text.replace(/\s+/g, " ").trim();
  if (normalized.length === 0) return [""];
  const words = normalized.split(" ");
  const lines: string[] = [];
  let current = "";
  const pushCurrent = () => {
    if (current.length > 0) lines.push(current);
    current = "";
  };
  for (const word of words) {
    if (word.length > width) {
      pushCurrent();
      for (let i = 0; i < word.length; i += width) {
        lines.push(word.slice(i, i + width));
      }
      continue;
    }
    if (current.length === 0) {
      current = word;
      continue;
    }
    if ((current.length + 1 + word.length) <= width) {
      current = `${current} ${word}`;
    } else {
      lines.push(current);
      current = word;
    }
  }
  pushCurrent();
  return lines;
}

export function readNestedValue(root: unknown, key: string): unknown {
  const parts = key.split(".");
  let cur: any = root;
  for (const p of parts) cur = cur[p];
  return cur;
}

export function writeNestedValue(root: unknown, key: string, value: unknown): unknown {
  const clone = structuredClone(root);
  const parts = key.split(".");
  let cursor: any = clone;
  for (let i = 0; i < parts.length - 1; i += 1) cursor = cursor[parts[i]];
  const leaf = parts[parts.length - 1];
  cursor[leaf] = typeof cursor[leaf] === "number" ? Number(value) : value;
  return clone;
}

export function readValue<TConfig extends object>(cfg: TConfig, key: string): unknown {
  const parts = key.split(".");
  let cur: any = cfg;
  for (const p of parts) cur = cur[p];
  return cur;
}

export function writeValue<TConfig extends object>(cfg: TConfig, key: string, value: unknown): TConfig {
  const clone = structuredClone(cfg) as TConfig;
  const parts = key.split(".");
  let cursor: any = clone;
  for (let i = 0; i < parts.length - 1; i += 1) cursor = cursor[parts[i]];
  cursor[parts[parts.length - 1]] = value;
  return clone;
}

export function deriveBusAutoName(bus: FxBusConfig): string {
  const parts: string[] = [];
  if (bus.slot1.type !== "none") parts.push(bus.slot1.type);
  if (bus.slot2.type !== "none") parts.push(bus.slot2.type);
  if (parts.length === 0) return "(none)";
  return parts.join("+");
}

export function derivePartAutoName(part: PartConfig): string {
  return part.l1.behaviorId;
}

export function deriveInstAutoName(instrument: { type: string }): string {
  if (instrument.type === "midi") return "MIDI";
  if (instrument.type === "sample") return "sample";
  if (instrument.type === "none") return "none";
  return "synth";
}

export function fxBusLabel(busIdx: number, bus: FxBusConfig): string {
  return `B${busIdx + 1}: ${bus.name}`;
}

export function partLabel(partIdx: number, part: PartConfig): string {
  return `P${partIdx + 1}: ${part.name}`;
}

export function instrumentLabel(state: { runtimeConfig: { instruments: Array<{ name: string }> } }, idx: number): string {
  const inst: any = (state.runtimeConfig as any).instruments?.[idx] ?? {};
  return `I${idx + 1}: ${inst.name ?? "synth"}`;
}

type FormatFn = (value: unknown, cfg?: RuntimeConfig) => string;

const CHANNEL_RE = /^instruments\.\d+\.midi\.channel$/;
const INSTR_TYPE_RE = /^instruments\.\d+\.type$/;
const SLOT_RE = /^instruments\.\d+\.sample\.selectedSlot$/;
const ROUTE_RE = /^instruments\.\d+\.mixer\.route$/;
const MAPPING_SLOT_RE = /\.l2\.mapping\.(activate|stable|deactivate|scanned|scanned_empty)\.slot$/;
const MAPPING_CHANNEL_RE = /^mapping\.(activate|stable|deactivate|scanned|scanned_empty)\.channel$/;
const PITCH_NOTE_RE = /^(?:pitch\.(?:startingNote|lowestNote|highestNote)|.+\.l2\.pitch\.(?:startingNote|lowestNote|highestNote))$/;
const SCAN_MODE_RE = /^(?:scanMode|.+\.l2\.scanMode)$/;
const SCAN_AXIS_RE = /^(?:scanAxis|.+\.l2\.scanAxis)$/;
const SCAN_DIR_RE = /^(?:scanDirection|.+\.l2\.scanDirection)$/;
const PITCH_OUT_RE = /^(?:pitch\.outOfRange|.+\.l2\.pitch\.outOfRange)$/;
const PITCH_SCALE_RE = /^(?:pitch\.scale|.+\.l2\.pitch\.scale)$/;
const PITCH_ROOT_RE = /^(?:pitch\.root|.+\.l2\.pitch\.root)$/;

const FORMAT_MAP: Array<[RegExp | string, FormatFn]> = [
  [MAPPING_CHANNEL_RE, (v, cfg) => routeOptionLabel(Number(v), cfg)],
  [MAPPING_SLOT_RE, (v, cfg) => routeOptionLabel(Number(v), cfg)],
  [CHANNEL_RE, (v) => String(clamp(Math.floor(Number(v)), 0, 15) + 1)],
  [INSTR_TYPE_RE, (v) => v === "midi" ? "MIDI only" : v === "sample" ? "sample" : v === "none" ? "none" : "synth"],
  [SLOT_RE, (v) => String(clamp(Math.floor(Number(v)), 0, PLATFORM_CAPS.sampleSlotCount - 1) + 1)],
  [ROUTE_RE, (v, cfg) => routeLabel(String(v), cfg)],
  [SCAN_MODE_RE, (v) => v === "immediate" ? "none" : "scanning"],
  [SCAN_AXIS_RE, (v) => v === "columns" ? "cols" : "rows"],
  [SCAN_DIR_RE, (v) => v === "forward" ? "fwd" : "rev"],
  [PITCH_NOTE_RE, (v) => formatNoteWithMidi(Number(v))],
  [PITCH_OUT_RE, (v) => v === "wrap" ? "wrap" : "clamp"],
  [PITCH_SCALE_RE, (v) => formatScaleName(String(v))],
  [PITCH_ROOT_RE, (v) => String(v)],
  ["masterVolume", (v) => `Vol: ${v}%`],
  ["displayBrightness", (v) => `OLED ${v}%`],
  ["gridBrightness", (v) => `Grid ${v}%`],
  ["buttonBrightness", (v) => `Btn ${v}%`],
  ["screenSleepSeconds", (v) => Number(v) <= 0 ? "Sleep: Off" : `Sleep: ${v}s`],
  ["activeBehavior", (v) => String(v)],
  ["activePartIndex", (v, cfg) => activePartLabel(Number(v), cfg)],
  ["transport.playing", (v) => v === true || v === "true" ? "Play" : "Stop"]
];

function routeOptionLabel(n: number, cfg?: RuntimeConfig): string {
  const idx = clamp(Math.floor(n), 0, 15);
  const inst = cfg?.instruments?.[idx];
  if (inst) return instrumentLabel({ runtimeConfig: cfg as any }, idx);
  return String(idx + 1);
}

function activePartLabel(n: number, cfg?: RuntimeConfig): string {
  const idx = clamp(Math.floor(n), 0, PLATFORM_CAPS.partCount - 1);
  const parts = cfg?.parts ?? [];
  if (parts[idx]) return partLabel(idx, parts[idx]);
  return `Part ${idx + 1}`;
}

function routeLabel(raw: string, cfg?: RuntimeConfig): string {
  if (raw === "direct") return "direct";
  const m = /^fx_bus_(\d+)$/.exec(raw);
  if (m && cfg?.mixer?.buses) {
    const busIdx = Number(m[1]) - 1;
    const bus = cfg.mixer.buses[busIdx];
    if (bus) return fxBusLabel(busIdx, bus);
  }
  return raw;
}

export function formatDisplayValue(key: string, value: unknown, runtimeConfig?: RuntimeConfig): string {
  for (const [pattern, fn] of FORMAT_MAP) {
    if (typeof pattern === "string" ? key === pattern : pattern.test(key)) {
      return fn(value, runtimeConfig);
    }
  }
  if (typeof value === "boolean") return value ? "On" : "Off";
  return String(value);
}

function formatNoteWithMidi(note: number): string {
  const n = clamp(Math.round(note), 0, 127);
  const names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
  const name = names[n % 12];
  const octave = Math.floor(n / 12) - 1;
  return `${name}${octave} (${n})`;
}

function formatScaleName(scale: string): string {
  const map: Record<string, string> = {
    chromatic: "Chromatic",
    major: "Major",
    natural_minor: "Natural Minor",
    dorian: "Dorian",
    mixolydian: "Mixolydian",
    major_pentatonic: "Maj Pentatonic",
    minor_pentatonic: "Min Pentatonic",
    harmonic_minor: "Harm Minor"
  };
  return map[scale] ?? scale;
}
