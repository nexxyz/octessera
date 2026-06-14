import type { CellTriggerIntent, CellTriggerKind } from "@cellsymphony/interpretation-core";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import defaults from "./config/default-mapping.json";

export type TriggerTarget = {
  action: "none" | "note_on" | "note_off";
  channel: number;
  velocity: number;
  durationMs: number;
};

export type RangeMode = "clamp" | "wrap";

export type MappingConfig = {
  baseMidiNote: number;
  maxMidiNote: number;
  rangeMode: RangeMode;
  scale: number[];
  rowStepDegrees: number;
  columnStepDegrees: number;
  activate: TriggerTarget;
  deactivate: TriggerTarget;
  stable: TriggerTarget;
  scanned: TriggerTarget;
  scanned_empty: TriggerTarget;
};

export function loadDefaultMappingConfig(): MappingConfig {
  return validateConfig(defaults as MappingConfig);
}

export function mapIntentsToMusicalEvents(intents: CellTriggerIntent[], config: MappingConfig): { events: MusicalEvent[]; intents: CellTriggerIntent[] } {
  const safe = validateConfig(config);
  const events: MusicalEvent[] = [];
  const matched: CellTriggerIntent[] = [];
  for (const intent of intents) {
    const note = noteFromDegree(intent.degree, safe);
    const target = targetForKind(intent.kind, safe);
    if (target.action === "none") continue;
    if (target.action === "note_off") {
      events.push({ type: "note_off", channel: target.channel, note });
      continue;
    }
    events.push({ type: "note_on", channel: target.channel, note, velocity: target.velocity, durationMs: target.durationMs });
    matched.push(intent);
  }
  return { events, intents: matched };
}

function targetForKind(kind: CellTriggerKind, config: MappingConfig): TriggerTarget {
  if (kind === "activate") return config.activate;
  if (kind === "deactivate") return config.deactivate;
  if (kind === "scanned_empty") return config.scanned_empty;
  if (kind === "scanned") return config.scanned;
  return config.stable;
}

function noteFromDegree(degree: number, config: MappingConfig): number {
  const note = degreeToPentatonicNote(degree, config);
  if (config.rangeMode === "wrap") {
    return wrapDegreeIntoRange(degree, config);
  }
  return clamp(note, config.baseMidiNote, config.maxMidiNote);
}

function degreeToPentatonicNote(degree: number, config: MappingConfig): number {
  const scaleLen = config.scale.length;
  const scaleIndex = mod(degree, scaleLen);
  const octave = Math.floor(degree / scaleLen);
  return config.baseMidiNote + octave * 12 + config.scale[scaleIndex];
}

function validateConfig(config: MappingConfig): MappingConfig {
  if (config.scale.length === 0) {
    throw new Error("Mapping scale must contain at least one degree.");
  }
  const baseMidiNote = clamp(Math.floor(config.baseMidiNote), 0, 127);
  const maxMidiNote = clamp(Math.floor(config.maxMidiNote), baseMidiNote, 127);
  return {
    ...config,
    baseMidiNote,
    maxMidiNote,
    rangeMode: config.rangeMode === "clamp" ? "clamp" : "wrap",
    scale: config.scale.map((step) => clamp(Math.floor(step), 0, 11)),
    rowStepDegrees: Math.max(0, Math.floor(config.rowStepDegrees)),
    columnStepDegrees: Math.max(0, Math.floor(config.columnStepDegrees)),
    activate: sanitizeTarget(config.activate),
    deactivate: sanitizeTarget(config.deactivate),
    stable: sanitizeTarget(config.stable ?? config.activate),
    scanned: sanitizeTarget(config.scanned ?? config.activate),
    scanned_empty: sanitizeTarget(config.scanned_empty ?? config.deactivate ?? config.activate)
  };
}

function sanitizeTarget(target: TriggerTarget): TriggerTarget {
  return {
    action: target.action === "note_off" || target.action === "none" ? target.action : "note_on",
    channel: clamp(Math.floor(target.channel), 0, 15),
    velocity: clamp(Math.floor(target.velocity), 1, 127),
    durationMs: clamp(Math.floor(target.durationMs), 1, 8000)
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function mod(value: number, base: number): number {
  return ((value % base) + base) % base;
}

function wrapDegreeIntoRange(degree: number, config: MappingConfig): number {
  const maxDegree = maxPentatonicDegreeInRange(config);
  if (maxDegree <= 0) {
    return clamp(config.baseMidiNote, config.baseMidiNote, config.maxMidiNote);
  }

  const wrappedDegree = mod(degree, maxDegree + 1);
  const wrappedNote = degreeToPentatonicNote(wrappedDegree, config);
  return clamp(wrappedNote, config.baseMidiNote, config.maxMidiNote);
}

function maxPentatonicDegreeInRange(config: MappingConfig): number {
  let degree = 0;
  while (degree < 2048) {
    const note = degreeToPentatonicNote(degree, config);
    if (note > config.maxMidiNote) {
      return Math.max(0, degree - 1);
    }
    degree += 1;
  }
  return 0;
}
