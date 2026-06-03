import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { RootName, RuntimeConfig, ScaleId, ValueLaneConfig } from "./platformTypes";
import { DEFAULT_VELOCITY_HIGH, DEFAULT_VELOCITY_MEDIUM, DEFAULT_VELOCITY_LOW, DEFAULT_NOTE_LENGTH_MS } from "./runtimeDefaults";
import { clampSampleSlotIndex, PLATFORM_CAPS, sectionCount } from "./platformCaps";
import { writeValue } from "./coreUtils";
import { paramModsForPart, scaledParamModValue } from "./paramMod";

export function applyModulation(intents: { x: number; y: number; degree: number; kind: any }[], events: MusicalEvent[], cfg: RuntimeConfig, partIndex: number = Number((cfg as any).activePartIndex ?? 0)): MusicalEvent[] {
  return applyModulationResult(intents, events, cfg, cfg, partIndex).events;
}

export function applyModulationResult(
  intents: { x: number; y: number; degree: number; kind: any }[],
  events: MusicalEvent[],
  eventCfg: RuntimeConfig,
  runtimeCfg: RuntimeConfig,
  partIndex: number = Number((runtimeCfg as any).activePartIndex ?? 0)
): { events: MusicalEvent[]; runtimeConfig: RuntimeConfig } {
    const out: MusicalEvent[] = [];
    for (let i = 0; i < events.length; i += 1) {
      const event = events[i];
     const intent = intents[i] ?? intents[intents.length - 1];
     if (!intent) {
       out.push(event);
       continue;
      }
      const targetChannel = event.type === "note_on" ? event.channel : 0;
      const ccs = ccFromIntent(intent, eventCfg, targetChannel);
      out.push(...ccs);
      if (event.type === "note_on") {
        const sampleResolved = resolveSampleAssignedNote(intent, eventCfg, event.channel, event.velocity, velocityFromIntent(intent, eventCfg));
        if (sampleResolved) {
          out.push({ ...event, note: sampleResolved.note, velocity: sampleResolved.velocity });
          continue;
        }
        if (isSampleInstrument(eventCfg, event.channel)) {
          continue;
        }
        const note = pitchFromIntent(intent, eventCfg, event.note);
        const vel = velocityFromIntent(intent, eventCfg);
        if (vel !== null) {
          out.push({ ...event, note, velocity: vel });
         continue;
       }
       out.push({ ...event, note });
        continue;
      }
      if (event.type === "note_off" && isSampleInstrument(eventCfg, event.channel)) {
        const assigned = resolveSampleAssignedNote(intent, eventCfg, event.channel, 100, null);
        if (!assigned) continue;
        out.push({ ...event, note: assigned.note });
        continue;
      }
      if (event.type === "note_off") {
        const note = pitchFromIntent(intent, eventCfg, event.note);
        out.push({ ...event, note });
        continue;
      }
      out.push(event);
    }
     const runtimeConfig = applyParamModulation(intents, runtimeCfg, partIndex);
     return { events: applyGlobalSound(out, runtimeConfig), runtimeConfig };
   }

export function applyParamModulation(intents: any[], cfg: RuntimeConfig, partIndex: number = Number((cfg as any).activePartIndex ?? 0)): RuntimeConfig {
   const intent = intents.find((i) => i && (i.kind === "activate" || i.kind === "scanned" || i.kind === "stable")) ?? intents[intents.length - 1];
   if (!intent) return cfg;
   const paramMods = paramModsForPart(cfg, partIndex);
   let next: RuntimeConfig = cfg;
   for (const axis of ["x", "y"] as const) {
     for (const slot of paramMods[axis]) {
       if (!slot) continue;
       next = writeParamModValue(next, slot.key, scaledParamModValue(slot, axis, intent), partIndex);
     }
   }
    return next;
  }

function writeParamModValue(cfg: RuntimeConfig, key: string, value: unknown, partIndex: number): RuntimeConfig {
   let next = writeValue(cfg, key, value);
   const partBehaviorConfigMatch = /^parts\.(\d+)\.l1\.behaviorConfig\.([^.]+)$/.exec(key);
   if (partBehaviorConfigMatch) {
     const idx = Number(partBehaviorConfigMatch[1]);
     const behaviorId = String((next as any).parts?.[idx]?.l1?.behaviorId ?? (next as any).activeBehavior);
     next = writeValue(next, `behaviorConfig.${behaviorId}.${partBehaviorConfigMatch[2]}`, value);
     if (idx === partIndex) {
       next = writeValue(next, "activeBehavior", behaviorId);
     }
   }
   return next;
}

function resolveSampleAssignedNote(
  intent: { x: number; y: number },
  cfg: RuntimeConfig,
  channel: number,
  sourceVelocity: number,
  senseVelocity: number | null
): { note: number; velocity: number } | null {
 const instruments: any[] = Array.isArray((cfg as any).instruments) ? ((cfg as any).instruments as any[]) : [];
   const inst = instruments[channel];
   if (!inst || inst.type !== "sampler") return null;
  const assignments: any[] = Array.isArray(inst.sample?.assignments) ? inst.sample.assignments : [];
  const a = assignments.find((x) => x.x === intent.x && x.y === intent.y);
  if (!a) return null;
  const base = sampleBaseVelocity(inst, a.level as any);
  const sense = clamp(Math.round(senseVelocity ?? sourceVelocity), 1, 127);
  const vel = clamp(Math.round((base * sense) / 127), 1, 127);
  const sampleSlot = clampSampleSlotIndex(a.sampleSlot ?? 0);
  return { note: 36 + sampleSlot, velocity: vel };
}

function sampleBaseVelocity(inst: any, level: "high" | "medium" | "low" | undefined): number {
  const sample = inst.sample ?? {};
  if (sample.velocityLevelsEnabled === true) {
    if (level === "high") return clamp(Math.round(Number(sample.velocityLevels?.high ?? DEFAULT_VELOCITY_HIGH)), 1, 127);
    if (level === "medium") return clamp(Math.round(Number(sample.velocityLevels?.medium ?? DEFAULT_VELOCITY_MEDIUM)), 1, 127);
    return clamp(Math.round(Number(sample.velocityLevels?.low ?? DEFAULT_VELOCITY_LOW)), 1, 127);
  }
  return clamp(Math.round(Number(sample.baseVelocity ?? 100)), 1, 127);
}

function isSampleInstrument(cfg: RuntimeConfig, channel: number): boolean {
   const instruments: any[] = Array.isArray((cfg as any).instruments) ? ((cfg as any).instruments as any[]) : [];
   return instruments[channel]?.type === "sampler";
}

export function applyGlobalSound(events: MusicalEvent[], cfg: RuntimeConfig): MusicalEvent[] {
  const sound = (cfg as any).sound;
  const scale = Math.max(0, Math.min(2, Number(sound?.velocityScalePct ?? 100) / 100));
  const curve: "linear" | "soft" | "hard" = sound?.velocityCurve ?? "linear";
  const noteLen = Math.max(1, Math.min(10_000, Number(sound?.noteLengthMs ?? DEFAULT_NOTE_LENGTH_MS)));

  return events.map((e) => {
    if (e.type !== "note_on") return e;
    const v0 = Math.max(1, Math.min(127, e.velocity));
    const n = v0 / 127;
    const shaped = curve === "soft" ? Math.sqrt(n) : curve === "hard" ? n * n : n;
    const v1 = Math.max(1, Math.min(127, Math.round(shaped * 127 * scale)));
    return { ...e, velocity: v1, durationMs: e.durationMs ?? noteLen };
  });
}

export function pitchFromIntent(intent: { x: number; y: number }, cfg: RuntimeConfig, fallbackNote: number): number {
  const pitchPos = sectionPitchPosition(intent, cfg);
  const xNorm = normalizedAxis(pitchPos.x, PLATFORM_CAPS.gridWidth, 0);
  const yNorm = normalizedAxis(pitchPos.y, PLATFORM_CAPS.gridHeight, 0);
  const xPos = Math.round(xNorm * (PLATFORM_CAPS.gridWidth - 1));
  const yPos = Math.round(yNorm * (PLATFORM_CAPS.gridHeight - 1));
  const xDelta = cfg.x.pitch.enabled ? xPos * cfg.x.pitch.steps : 0;
  const yDelta = cfg.y.pitch.enabled ? yPos * cfg.y.pitch.steps : 0;
  if (!cfg.x.pitch.enabled && !cfg.y.pitch.enabled) return fallbackNote;
  const low = Math.min(cfg.pitch.lowestNote, cfg.pitch.highestNote);
  const high = Math.max(cfg.pitch.lowestNote, cfg.pitch.highestNote);
  const scaleNotes = buildScaleNotes(cfg.pitch.scale, cfg.pitch.root, low, high);
  if (scaleNotes.length === 0) return clamp(fallbackNote, low, high);
  const startIndex = nearestScaleIndex(scaleNotes, cfg.pitch.startingNote);
  let targetIndex = startIndex + xDelta + yDelta;
  if (cfg.pitch.outOfRange === "clamp") {
    targetIndex = clamp(targetIndex, 0, scaleNotes.length - 1);
  } else {
    targetIndex = mod(targetIndex, scaleNotes.length);
  }
  return scaleNotes[targetIndex] ?? clamp(fallbackNote, low, high);
}

function sectionPitchPosition(intent: { x: number; y: number }, cfg: RuntimeConfig): { x: number; y: number } {
  const sections = sectionCount(cfg.scanSections);
  if (cfg.scanMode !== "scanning" || sections <= 1) return intent;
  if (cfg.scanAxis === "rows" && cfg.y.pitch.restartEachSection) {
    const sectionHeight = Math.max(1, Math.floor(PLATFORM_CAPS.gridHeight / sections));
    return { x: intent.x, y: intent.y % sectionHeight };
  }
  if (cfg.scanAxis === "columns" && cfg.x.pitch.restartEachSection) {
    const sectionWidth = Math.max(1, Math.floor(PLATFORM_CAPS.gridWidth / sections));
    return { x: intent.x % sectionWidth, y: intent.y };
  }
  return intent;
}

function velocityFromIntent(intent: { x: number; y: number }, cfg: RuntimeConfig): number | null {
  const vals: number[] = [];
  if (cfg.x.velocity.enabled) vals.push(valueFromAxis(intent.x, PLATFORM_CAPS.gridWidth, cfg.x.velocity));
  if (cfg.y.velocity.enabled) vals.push(valueFromAxis(intent.y, PLATFORM_CAPS.gridHeight, cfg.y.velocity));
  if (vals.length === 0) return null;
  return clamp(Math.round(vals.reduce((a, b) => a + b, 0) / vals.length), 1, 127);
}

function ccFromIntent(intent: { x: number; y: number }, cfg: RuntimeConfig, channel: number): MusicalEvent[] {
  const events: MusicalEvent[] = [];
  const pushCc = (controller: number, source: number, min: number, max: number) => {
    const scaled = clamp(Math.round(min + source * (max - min)), 0, 127);
    events.push({ type: "cc", channel: clamp(channel, 0, 15), controller, value: scaled });
  };
  if (cfg.x.filterCutoff.enabled) pushCc(74, normalizedAxis(intent.x, PLATFORM_CAPS.gridWidth, cfg.x.filterCutoff.gridOffset), cfg.x.filterCutoff.from, cfg.x.filterCutoff.to);
  if (cfg.y.filterCutoff.enabled) pushCc(74, normalizedAxis(intent.y, PLATFORM_CAPS.gridHeight, cfg.y.filterCutoff.gridOffset), cfg.y.filterCutoff.from, cfg.y.filterCutoff.to);
  if (cfg.x.filterResonance.enabled) pushCc(71, normalizedAxis(intent.x, PLATFORM_CAPS.gridWidth, cfg.x.filterResonance.gridOffset), cfg.x.filterResonance.from, cfg.x.filterResonance.to);
  if (cfg.y.filterResonance.enabled) pushCc(71, normalizedAxis(intent.y, PLATFORM_CAPS.gridHeight, cfg.y.filterResonance.gridOffset), cfg.y.filterResonance.from, cfg.y.filterResonance.to);
  return events;
}

function valueFromAxis(index: number, size: number, lane: ValueLaneConfig): number {
  const norm = normalizedAxis(index, size, lane.gridOffset);
  return lane.from + norm * (lane.to - lane.from);
}

function normalizedAxis(index: number, size: number, gridOffset: number): number {
  const shifted = mod(index + gridOffset, size);
  return shifted / Math.max(1, size - 1);
}

function buildScaleNotes(scale: ScaleId, root: RootName, low: number, high: number): number[] {
  const intervals = scaleIntervals(scale);
  const rootPc = rootPitchClass(root);
  const notes: number[] = [];
  for (let n = clamp(low, 0, 127); n <= clamp(high, 0, 127); n += 1) {
    const pc = mod(n - rootPc, 12);
    if (intervals.includes(pc)) notes.push(n);
  }
  return notes;
}

function nearestScaleIndex(notes: number[], target: number): number {
  let bestIdx = 0;
  let bestDist = Number.POSITIVE_INFINITY;
  for (let i = 0; i < notes.length; i += 1) {
    const d = Math.abs(notes[i] - target);
    if (d < bestDist) {
      bestDist = d;
      bestIdx = i;
    }
  }
  return bestIdx;
}

function rootPitchClass(root: RootName): number {
  const map: Record<RootName, number> = {
    C: 0,
    "C#": 1,
    D: 2,
    "D#": 3,
    E: 4,
    F: 5,
    "F#": 6,
    G: 7,
    "G#": 8,
    A: 9,
    "A#": 10,
    B: 11
  };
  return map[root];
}

function scaleIntervals(scale: ScaleId): number[] {
  switch (scale) {
    case "chromatic":
      return [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    case "major":
      return [0, 2, 4, 5, 7, 9, 11];
    case "natural_minor":
      return [0, 2, 3, 5, 7, 8, 10];
    case "dorian":
      return [0, 2, 3, 5, 7, 9, 10];
    case "mixolydian":
      return [0, 2, 4, 5, 7, 9, 10];
    case "major_pentatonic":
      return [0, 2, 4, 7, 9];
    case "minor_pentatonic":
      return [0, 3, 5, 7, 10];
    case "harmonic_minor":
      return [0, 2, 3, 5, 7, 8, 11];
  }
  return [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
}

export function applyNoteBehavior(
  events: MusicalEvent[],
  instruments: any[],
  partIdx: number,
  initialHeld: string[]
): { events: MusicalEvent[]; heldNotes: string[] } {
  const held = new Set(initialHeld);
  const out: MusicalEvent[] = [];
  for (const e of events) {
    if (e.type === "note_on") {
      const key = `${partIdx}:${e.channel}:${e.note}`;
      const behavior = instruments[e.channel]?.noteBehavior === "hold" ? "hold" : "oneshot";
      if (behavior === "hold" && held.has(key)) continue;
      if (behavior === "hold") {
        held.add(key);
        out.push({ ...e, durationMs: undefined });
      } else {
        out.push(e);
      }
    } else if (e.type === "note_off") {
      const key = `${partIdx}:${e.channel}:${e.note}`;
      if (!held.has(key)) { out.push(e); continue; }
      held.delete(key);
      out.push(e);
    } else {
      out.push(e);
    }
  }
  return { events: out, heldNotes: [...held] };
}

export function withScaleSteps(mapping: any, cfg: RuntimeConfig): any {
  return {
    ...mapping,
    rowStepDegrees: cfg.y.pitch.enabled ? Math.abs(cfg.y.pitch.steps) : 0,
    columnStepDegrees: cfg.x.pitch.enabled ? Math.abs(cfg.x.pitch.steps) : 0
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function mod(value: number, base: number): number {
  return ((value % base) + base) % base;
}
