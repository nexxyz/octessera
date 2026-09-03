import {
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
  PAN_POSITION_COUNT,
  type NeoKeyLeds,
  type OledFrame,
  type LocalBootstrapSnapshot,
  type NativeRuntimeSnapshot,
  type RuntimeSnapshot,
  type RuntimeStatus,
} from '@octessera/device-contracts';
import type { SimulatorSnapshot } from './types';
import type { OledFrameCacheFault } from './oledFrameCache';

export type RuntimeSnapshotCache = {
  audioRevision?: number;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  masterVolume: number;
};

export type SnapshotAudioState = {
  audioLoad: import('../audio/audioLoadEvents').AudioLoadStatus;
  runtimeStatus: RuntimeStatus | null;
  oledFrameFault?: OledFrameCacheFault | null;
  oledFrameAvailable?: boolean;
};

export function createRuntimeSnapshotCache(): RuntimeSnapshotCache {
  return {
    instruments: [],
    mixer: { buses: [] },
    panPositions: PAN_POSITION_COUNT,
    masterVolume: 100,
  };
}

export function createInitialRuntimeSnapshot(): LocalBootstrapSnapshot {
  const blankOled: OledFrame = {
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be',
    pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2),
  };
  const ledCount = GRID_WIDTH * GRID_HEIGHT;
  return {
    oled: blankOled,
    oledFrameRevision: 0,
    leds: {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      rgb: Array.from({ length: ledCount * 3 }, () => 0),
      active: Array.from({ length: ledCount }, () => false),
    },
    transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
    display: {
      page: 'boot',
      bodyLayout: 'rows',
      title: 'Boot',
      lines: [],
      editing: false,
    },
    activeBehavior: 'life',
    gridInteraction: 'paint',
    neoKeyLeds: {
      back: [221, 130, 205],
      space: [221, 130, 205],
      shift: [67, 68, 71],
      fn: [67, 68, 71],
    },
    eventDotOn: false,
    transportIcon: 'stop',
    transportFlash: 'none',
  };
}

export function mergeSnapshotSettings(
  snapshot: NativeRuntimeSnapshot,
  previous: RuntimeSnapshot | LocalBootstrapSnapshot,
): void {
  const previousSettings = previous.settings;
  const nextSettings = snapshot.settings;
  if (!previousSettings || !nextSettings) return;
  if (!('instruments' in nextSettings))
    nextSettings.instruments = previousSettings.instruments;
  if (!('mixer' in nextSettings)) nextSettings.mixer = previousSettings.mixer;
  if (!('panPositions' in nextSettings))
    nextSettings.panPositions = previousSettings.panPositions;
}

export function snapshotFromCore(
  frame: RuntimeSnapshot | LocalBootstrapSnapshot,
  cache: RuntimeSnapshotCache,
  audio: SnapshotAudioState,
  oled: OledFrame,
): SimulatorSnapshot {
  const settings = frame.settings;
  refreshRuntimeSnapshotCache(cache, settings);
  const frameWithoutRevision = { ...frame };
  if ('oledFrameRevision' in frameWithoutRevision) {
    delete (frameWithoutRevision as { oledFrameRevision?: unknown })
      .oledFrameRevision;
  }
  return {
    frame: { ...frameWithoutRevision, oled },
    runtimeStatus: audio.runtimeStatus,
    oledFrameFault: audio.oledFrameFault ?? null,
    oledFrameAvailable: audio.oledFrameAvailable ?? false,
    neoKeyLeds: scaleNeoKeyLeds(
      frame.neoKeyLeds,
      settings?.buttonBrightness,
      settings?.ledsDimmed ?? false,
    ),
    displayBrightness: settings?.displayBrightness ?? 75,
    buttonBrightness: settings?.buttonBrightness ?? 75,
    masterVolume: cache.masterVolume,
    voiceStealingMode: settings?.voiceStealingMode ?? 'auto-balanced',
    audioLoad: audio.audioLoad,
    instruments: cache.instruments,
    mixer: cache.mixer,
    panPositions: cache.panPositions,
    autoSaveFlash: settings?.autoSaveFlash ?? 'none',
    autoSaveFlashSerial: settings?.autoSaveFlashSerial,
  };
}

function refreshRuntimeSnapshotCache(
  cache: RuntimeSnapshotCache,
  settings: RuntimeSnapshot['settings'],
): void {
  if (!settings) return;
  const revision = settings.audioConfigRevision;
  if (
    cache.audioRevision !== undefined &&
    revision !== undefined &&
    revision === cache.audioRevision
  ) {
    return;
  }
  cache.audioRevision = revision;
  cache.instruments = settings.instruments ?? [];
  cache.mixer = settings.mixer ?? { buses: [] };
  cache.panPositions = settings.panPositions ?? PAN_POSITION_COUNT;
  cache.masterVolume = settings.masterVolume ?? 100;
}

export function scaleNeoKeyLeds(
  leds: NeoKeyLeds,
  buttonBrightness: number | undefined,
  dimmed: boolean,
): SimulatorSnapshot['neoKeyLeds'] {
  const brightness = Math.min(
    100,
    Math.max(0, Math.trunc(buttonBrightness ?? 100)),
  );
  const scale = dimmed
    ? brightness === 0
      ? 0
      : Math.max(brightness * 8, 400)
    : brightness * 100;
  const scaleColor = (
    rgb: [number, number, number],
  ): [number, number, number] =>
    rgb.map((channel) => Math.floor((channel * scale + 5000) / 10000)) as [
      number,
      number,
      number,
    ];
  return {
    back: scaleColor(leds.back),
    space: scaleColor(leds.space),
    shift: scaleColor(leds.shift),
    fn: scaleColor(leds.fn),
  };
}
