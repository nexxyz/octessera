import { PAN_POSITION_COUNT, type OledFrame, type RuntimeSnapshot } from "@octessera/device-contracts";
import type { SimulatorSnapshot } from "./types";

export type RuntimeSnapshotCache = {
  audioRevision?: number;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  masterVolume: number;
};

export type TransientIndicatorState = {
  eventDotUntilMs: number;
  transportFlashUntilMs: number;
  transportFlash: "measure" | "beat" | "none";
};

export type SnapshotAudioState = {
  audioLoad: { ratio: number; voiceSteal: boolean };
  audioError: string | null;
};

export function createRuntimeSnapshotCache(): RuntimeSnapshotCache {
  return {
    instruments: [],
    mixer: { buses: [] },
    panPositions: PAN_POSITION_COUNT,
    masterVolume: 100,
  };
}

export function createInitialRuntimeSnapshot(): RuntimeSnapshot {
  const blankOled: OledFrame = { width: 128, height: 128, format: "rgb565be", pixels: new Uint8Array(32768) };
  return {
    oled: blankOled,
    leds: { width: 8, height: 8, rgb: Array.from({ length: 64 * 3 }, () => 0), active: Array.from({ length: 64 }, () => false) },
    transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
    display: { page: "boot", title: "Boot", lines: [], editing: false },
    activeBehavior: "life",
    gridInteraction: "paint",
  };
}

export function normalizeSnapshotPixels(snapshot: RuntimeSnapshot): void {
  if (snapshot.oled && !(snapshot.oled.pixels instanceof Uint8Array)) {
    snapshot.oled = { ...snapshot.oled, pixels: new Uint8Array(Object.values(snapshot.oled.pixels as Record<string, number>)) };
  }
}

export function mergeSnapshotSettings(snapshot: RuntimeSnapshot, previous: RuntimeSnapshot): void {
  const previousSettings = previous.settings;
  const nextSettings = snapshot.settings;
  if (!previousSettings || !nextSettings) return;
  if (!("instruments" in nextSettings)) nextSettings.instruments = previousSettings.instruments;
  if (!("mixer" in nextSettings)) nextSettings.mixer = previousSettings.mixer;
  if (!("panPositions" in nextSettings)) nextSettings.panPositions = previousSettings.panPositions;
}

export function snapshotFromCore(
  frame: RuntimeSnapshot,
  cache: RuntimeSnapshotCache,
  shiftActive: boolean,
  indicators: TransientIndicatorState,
  audio: SnapshotAudioState,
): SimulatorSnapshot {
  const settings = frame.settings;
  const audioRevision = settings?.audioConfigRevision;
  if (settings && (cache.audioRevision === undefined || audioRevision === undefined || audioRevision !== cache.audioRevision)) {
    cache.audioRevision = audioRevision;
    cache.instruments = settings.instruments ?? [];
    cache.mixer = settings.mixer ?? { buses: [] };
    cache.panPositions = settings.panPositions ?? PAN_POSITION_COUNT;
    cache.masterVolume = settings.masterVolume ?? 100;
  }
  const flash = performance.now() < indicators.transportFlashUntilMs
    ? indicators.transportFlash
    : String(frame.transportFlash ?? "none");
  const transportIcon = String(frame.transportIcon ?? (frame.transport.playing ? "play" : "stop"));
  const space = transportIcon === "stop"
    ? "stopped"
    : transportIcon === "pause"
      ? "paused"
      : flash === "measure"
        ? "measure"
        : flash === "beat"
          ? "beat"
          : "playing";
  const neoKeyLeds = neoKeyColors(frame, space, settings?.buttonBrightness, shiftActive);
  return {
    frame: withTransientIndicators(frame, indicators),
    neoKeyLeds,
    displayBrightness: settings?.displayBrightness ?? 75,
    buttonBrightness: settings?.buttonBrightness ?? 75,
    masterVolume: cache.masterVolume,
    voiceStealingMode: settings?.voiceStealingMode ?? "auto-balanced",
    audioLoad: audio.audioLoad,
    audioError: audio.audioError,
    instruments: cache.instruments,
    mixer: cache.mixer,
    panPositions: cache.panPositions,
    audioConfigRevision: cache.audioRevision,
    autoSaveFlash: settings?.autoSaveFlash ?? "none",
    autoSaveFlashSerial: settings?.autoSaveFlashSerial,
  };
}

function neoKeyColors(
  frame: RuntimeSnapshot,
  space: "stopped" | "paused" | "playing" | "beat" | "measure",
  buttonBrightness: number | undefined,
  shiftActive: boolean,
): SimulatorSnapshot["neoKeyLeds"] {
  const settings = frame.settings;
  const scaleFactor = (settings?.ledsDimmed ? 0.08 : 1) * brightnessScale(buttonBrightness);
  const combined = settings?.combinedModifierHeld ?? false;
  return {
    back: scale([90, 0, 0], scaleFactor),
    space: scale(spaceColor(space), scaleFactor),
    shift: scale(combined ? [0, 0, 180] : (settings?.shiftHeld ?? shiftActive) ? [180, 140, 0] : [90, 45, 0], scaleFactor),
    fn: scale(combined ? [0, 0, 180] : (settings?.fnHeld ?? false) ? [180, 140, 0] : [90, 45, 0], scaleFactor),
  };
}

function spaceColor(space: "stopped" | "paused" | "playing" | "beat" | "measure"): [number, number, number] {
  if (space === "stopped") return [255, 0, 0];
  if (space === "paused") return [215, 255, 232];
  if (space === "measure") return [255, 160, 0];
  if (space === "beat") return [51, 255, 102];
  return [0, 80, 0];
}

function brightnessScale(value: number | undefined): number {
  return value === undefined ? 1 : Math.min(100, Math.max(0, value)) / 100;
}

function scale(rgb: [number, number, number], factor: number): [number, number, number] {
  return rgb.map((channel) => Math.round(channel * factor)) as [number, number, number];
}

function withTransientIndicators(frame: RuntimeSnapshot, indicators: TransientIndicatorState): RuntimeSnapshot {
  const transientEventDotOn = performance.now() < indicators.eventDotUntilMs;
  const transientTransport = performance.now() < indicators.transportFlashUntilMs ? indicators.transportFlash : null;
  if (!transientEventDotOn && transientTransport === null) return frame;
  return {
    ...frame,
    ...(transientEventDotOn ? { eventDotOn: true } : {}),
    ...(transientTransport ? { transportFlash: transientTransport } : {}),
  };
}
