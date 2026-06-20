import { cutoffDisplayToHz } from "@cellsymphony/device-contracts";

export type AudioConfigPayload = { instruments: unknown[]; mixer: unknown; panPositions: number; masterVolume: number };

function convertInstrumentForEngine(inst: unknown): unknown {
  if (typeof inst !== "object" || inst === null) return inst;
  const out = { ...(inst as Record<string, unknown>) };

  for (const prefix of ["synth", "sample"]) {
    const section = out[prefix] as Record<string, unknown> | undefined;
    if (!section) continue;
    const filter = section.filter as Record<string, unknown> | undefined;
    if (!filter) continue;
    if (typeof filter.cutoffHz === "number" && filter.cutoffHz <= 255) {
      out[prefix] = { ...section, filter: { ...filter, cutoffHz: cutoffDisplayToHz(filter.cutoffHz) } };
    }
  }

  return out;
}

function normalizeForEngine(config: AudioConfigPayload): AudioConfigPayload {
  return {
    ...config,
    instruments: (config.instruments as unknown[]).map(convertInstrumentForEngine)
  };
}

function audioConfigSignature(config: AudioConfigPayload): string {
  return JSON.stringify(config);
}

export function createCoalescedAudioConfigSender(
  send: (config: AudioConfigPayload) => void | Promise<void>,
  delayMs = 16
) {
  let pending: AudioConfigPayload | null = null;
  let pendingSignature = "";
  let lastSentSignature = "";
  let timer: ReturnType<typeof setTimeout> | null = null;

  function flush() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    if (!pending) return;
    const next = pending;
    const signature = pendingSignature;
    pending = null;
    pendingSignature = "";
    lastSentSignature = signature;
    void send(next);
  }

  function schedule(config: AudioConfigPayload): boolean {
    const normalized = normalizeForEngine(config);
    const newSignature = audioConfigSignature(normalized);
    if (pendingSignature === newSignature || lastSentSignature === newSignature) return false;
    pending = normalized;
    pendingSignature = newSignature;
    if (timer !== null) clearTimeout(timer);
    timer = setTimeout(flush, delayMs);
    return true;
  }

  function cancel() {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    pending = null;
    pendingSignature = "";
  }

  return { schedule, flush, cancel };
}
