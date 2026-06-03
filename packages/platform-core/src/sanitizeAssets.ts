import { isBusEffectType, sanitizeFxParams } from "./fxDefaults";
import { cutoffHzToDisplay } from "./coreUtils";
import { clampPanPosition, PLATFORM_CAPS } from "./platformCaps";
import { DEFAULT_VELOCITY_LEVELS, DEFAULT_MIDI_ENGINE, DEFAULT_PAN_POS, DEFAULT_VOLUME } from "./runtimeDefaults";

const BUS_EFFECT_TYPES = new Set([
  "none", "reverb", "delay", "tremolo", "vibrato", "auto_pan",
  "chorus", "flanger", "wah", "filter_lfo", "duck", "bitcrusher",
  "saturator", "distortion", "glitch", "compressor", "eq"
]);

export function normalizeSlot(raw: unknown): { type: string; params: Record<string, unknown> } {
  if (typeof raw === "string") {
    const type = BUS_EFFECT_TYPES.has(raw) ? raw : "none";
    return { type, params: sanitizeFxParams(type, {}) };
  }
  const typeRaw = typeof (raw as any)?.type === "string" ? (raw as any).type : "none";
  const type = isBusEffectType(typeRaw) && BUS_EFFECT_TYPES.has(typeRaw) ? typeRaw : "none";
  return { type, params: sanitizeFxParams(type, (raw as any)?.params) };
}

export function sanitizeMixer(incoming: any, factory: any): { buses: any[] } {
  const factoryMixer = (factory.runtimeConfig as any).mixer;
  const sourceBuses = Array.isArray(incoming?.buses) ? incoming.buses : (Array.isArray(factoryMixer?.buses) ? factoryMixer.buses : []);
  const buses: any[] = [];
  for (let i = 0; i < PLATFORM_CAPS.busCount; i += 1) {
    const src = sourceBuses[i] ?? {};
    const autoName = typeof src.autoName === "boolean" ? src.autoName : true;
    const srcName = typeof src.name === "string" && src.name.trim().length > 0 ? src.name.trim() : "(none)";
    buses.push({
      slot1: normalizeSlot(src.slot1),
      slot2: normalizeSlot(src.slot2),
      panPos: clampPanPosition(src.panPos ?? DEFAULT_PAN_POS),
      autoName,
      name: srcName
    });
  }
  return { buses };
}

export function sanitizeInstruments(incoming: unknown, factory: any): any[] {
  const factorySlots: any[] = Array.isArray((factory.runtimeConfig as any).instruments)
    ? (factory.runtimeConfig as any).instruments
    : [];
  const fallbackSlot = { type: "synth", midi: { enabled: false, channel: 0 }, synth: {}, sample: { baseVelocity: 100, velocityLevelsEnabled: false, velocityLevels: { ...DEFAULT_VELOCITY_LEVELS }, selectedSlot: 0, slots: Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, () => ({ path: null })), tuneSemis: 0, amp: {}, ampEnv: {}, filter: {}, filterEnv: {}, assignments: [] }, midiEngine: { ...DEFAULT_MIDI_ENGINE }, mixer: { route: "direct", panPos: DEFAULT_PAN_POS, volume: DEFAULT_VOLUME } };
  const baseSlots = factorySlots.length > 0 ? factorySlots : Array.from({ length: PLATFORM_CAPS.instrumentCount }, () => fallbackSlot);
  const src = Array.isArray(incoming) ? incoming : [];
  const out: any[] = [];
  for (let i = 0; i < PLATFORM_CAPS.instrumentCount; i += 1) {
    const f = baseSlots[i] ?? fallbackSlot;
    const s = src[i] ?? {};
    const incomingAutoName = typeof (s as any).autoName === "boolean" ? (s as any).autoName : true;
    const incomingName = typeof (s as any).name === "string" && (s as any).name.trim().length > 0 ? (s as any).name.trim() : "";
    const fallbackAutoName = typeof (f as any).autoName === "boolean" ? (f as any).autoName : true;
    const fallbackName = typeof (f as any).name === "string" && (f as any).name.trim().length > 0 ? (f as any).name.trim() : "";
    out.push({
    ...(f as any),
     ...(s as any),
     type: (s as any).type === "sampler" || (s as any).type === "midi" || (s as any).type === "synth" || (s as any).type === "none" ? (s as any).type : (f as any).type,
      autoName: incomingAutoName,
      name: incomingName || fallbackName || (f as any).name || "synth",
      midi: { ...(f as any).midi, ...((s as any).midi ?? {}) },
      synth: {
        ...(f as any).synth,
        ...((s as any).synth ?? {}),
        osc1: { ...(f as any).synth?.osc1, ...((s as any).synth?.osc1 ?? {}) },
        osc2: { ...(f as any).synth?.osc2, ...((s as any).synth?.osc2 ?? {}) },
        amp: { ...(f as any).synth?.amp, ...((s as any).synth?.amp ?? {}) },
        ampEnv: { ...(f as any).synth?.ampEnv, ...((s as any).synth?.ampEnv ?? {}) },
        filter: { ...(f as any).synth?.filter, ...((s as any).synth?.filter ?? {}) },
        filterEnv: { ...(f as any).synth?.filterEnv, ...((s as any).synth?.filterEnv ?? {}) }
      },
      sample: {
        ...(f as any).sample,
        ...((s as any).sample ?? {}),
        velocityLevels: { ...(f as any).sample?.velocityLevels, ...((s as any).sample?.velocityLevels ?? {}) },
        slots: (() => {
          const incomingSlots = Array.isArray((s as any).sample?.slots)
            ? (s as any).sample.slots.slice(0, PLATFORM_CAPS.sampleSlotCount).map((entry: any) => ({ path: typeof entry?.path === "string" ? entry.path : null }))
            : (Array.isArray((f as any).sample?.slots) ? (f as any).sample.slots.slice(0, PLATFORM_CAPS.sampleSlotCount).map((entry: any) => ({ path: typeof entry?.path === "string" ? entry.path : null })) : []);
          while (incomingSlots.length < PLATFORM_CAPS.sampleSlotCount) incomingSlots.push({ path: null });
          return incomingSlots;
        })(),
        assignments: Array.isArray((s as any).sample?.assignments) ? (s as any).sample.assignments : (Array.isArray((f as any).sample?.assignments) ? (f as any).sample.assignments : []),
        amp: { ...(f as any).sample?.amp, ...((s as any).sample?.amp ?? {}) },
        ampEnv: { ...(f as any).sample?.ampEnv, ...((s as any).sample?.ampEnv ?? {}) },
        filter: { ...(f as any).sample?.filter, ...((s as any).sample?.filter ?? {}) },
        filterEnv: { ...(f as any).sample?.filterEnv, ...((s as any).sample?.filterEnv ?? {}) }
      },
      midiEngine: {
        ...(f as any).midiEngine,
        ...((s as any).midiEngine ?? {})
      },
      mixer: {
        route: (() => {
          const raw = String((s as any).mixer?.route ?? (f as any).mixer?.route ?? "direct");
          if (raw === "direct") return "direct";
          const m = /^(?:fx_)?bus_(\d+)$/.exec(raw);
          if (!m) return "direct";
          const idx = Number(m[1]);
          if (!Number.isFinite(idx) || idx < 1 || idx > PLATFORM_CAPS.busCount) return "direct";
          return `fx_bus_${idx}`;
        })(),
        panPos: clampPanPosition((s as any).mixer?.panPos ?? (f as any).mixer?.panPos ?? DEFAULT_PAN_POS),
        volume: Math.max(0, Math.min(100, Number((s as any).mixer?.volume ?? (f as any).mixer?.volume ?? DEFAULT_VOLUME)))
      }
   });
     const inst = out[i] as Record<string, any>;
     for (const prefix of ["synth", "sampler"]) {
      const section = inst[prefix] as Record<string, any> | undefined;
      const filter = section?.filter;
      if (filter && typeof filter.cutoffHz === "number" && filter.cutoffHz > 255) {
        section.filter = { ...filter, cutoffHz: cutoffHzToDisplay(filter.cutoffHz) };
      }
    }
  }
  return out;
}
