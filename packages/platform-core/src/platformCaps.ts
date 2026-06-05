import { createGridDomain, type GridDomain } from "@cellsymphony/device-contracts";
import capsJson from "../../../config/platform-capabilities.json";

export type PlatformCapabilities = {
  gridWidth: number;
  gridHeight: number;
  partCount: number;
  instrumentCount: number;
  sampleSlotCount: number;
  busCount: number;
  globalFxSlotCount: number;
  touchFxMaxConcurrent: number;
  scanSectionCounts: number[];
};

function toPositiveInt(value: unknown, key: keyof PlatformCapabilities): number {
  const n = Number(value);
  if (!Number.isInteger(n) || n <= 0) {
    throw new Error(`Invalid platform capability '${key}': expected positive integer, got ${String(value)}`);
  }
  return n;
}

export function validatePlatformCapabilities(raw: unknown): PlatformCapabilities {
  const source = (raw ?? {}) as Record<string, unknown>;
  const rawSections = source.scanSectionCounts;
  const scanSectionCounts = Array.isArray(rawSections) && rawSections.length > 0
    ? (rawSections as unknown[]).map((v) => { const n = Number(v); if (!Number.isInteger(n) || n <= 0) throw new Error(`Invalid scanSectionCount entry: ${String(v)}`); return n; })
    : [1];
  return {
    gridWidth: toPositiveInt(source.gridWidth, "gridWidth"),
    gridHeight: toPositiveInt(source.gridHeight, "gridHeight"),
    partCount: toPositiveInt(source.partCount, "partCount"),
    instrumentCount: toPositiveInt(source.instrumentCount, "instrumentCount"),
    sampleSlotCount: toPositiveInt(source.sampleSlotCount, "sampleSlotCount"),
    busCount: toPositiveInt(source.busCount, "busCount"),
    globalFxSlotCount: toPositiveInt(source.globalFxSlotCount, "globalFxSlotCount"),
    touchFxMaxConcurrent: toPositiveInt(source.touchFxMaxConcurrent, "touchFxMaxConcurrent"),
    scanSectionCounts
  };
}

export const PLATFORM_CAPS: PlatformCapabilities = validatePlatformCapabilities(capsJson);
export const PAN_POSITION_COUNT = 33;
export const PAN_POSITION_MAX = PAN_POSITION_COUNT - 1;
export const PAN_CENTER_POS = Math.floor(PAN_POSITION_MAX / 2);

export function clampPanPosition(value: unknown): number {
  const n = Math.round(Number(value));
  return Math.max(0, Math.min(PAN_POSITION_MAX, Number.isFinite(n) ? n : PAN_CENTER_POS));
}

export function scalePanPosition(value: unknown, sourceCount: number): number {
  const count = Math.max(1, Math.floor(Number(sourceCount)));
  const pos = Math.max(0, Math.min(count - 1, Math.round(Number(value))));
  if (!Number.isFinite(pos)) return PAN_CENTER_POS;
  if (count <= 1) return PAN_CENTER_POS;
  return Math.max(0, Math.min(PAN_POSITION_MAX, Math.round((pos / (count - 1)) * PAN_POSITION_MAX)));
}

export function clampPartIndex(value: unknown): number {
  return Math.max(0, Math.min(PLATFORM_CAPS.partCount - 1, Number(value) | 0));
}

export function clampInstrumentIndex(value: unknown): number {
  return Math.max(0, Math.min(PLATFORM_CAPS.instrumentCount - 1, Number(value) | 0));
}

export function clampSampleSlotIndex(value: unknown): number {
  return Math.max(0, Math.min(PLATFORM_CAPS.sampleSlotCount - 1, Number(value) | 0));
}

export function partIndexOptions(): string[] {
  return Array.from({ length: PLATFORM_CAPS.partCount }, (_, i) => String(i));
}

export function instrumentIndexOptions(): string[] {
  return Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, i) => String(i));
}

export function sampleSlotOptions(): string[] {
  return Array.from({ length: PLATFORM_CAPS.sampleSlotCount }, (_, i) => String(i));
}

export function gridWidth(): number {
  return PLATFORM_CAPS.gridWidth;
}

export function gridHeight(): number {
  return PLATFORM_CAPS.gridHeight;
}

export function gridCellCount(): number {
  return PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight;
}

let _domain: GridDomain | null = null;

function getOrCreateDomain(): GridDomain {
  if (!_domain || _domain.width !== PLATFORM_CAPS.gridWidth || _domain.height !== PLATFORM_CAPS.gridHeight) {
    _domain = createGridDomain(PLATFORM_CAPS.gridWidth, PLATFORM_CAPS.gridHeight);
  }
  return _domain;
}

export function gridDomain(): GridDomain {
  return getOrCreateDomain();
}

export const GRID_DOMAIN: GridDomain = new Proxy({} as GridDomain, {
  get(_target, prop: string | symbol) {
    return (getOrCreateDomain() as any)[prop];
  }
});

export function scanSectionOptions(): string[] {
  return PLATFORM_CAPS.scanSectionCounts.map(String);
}

export function sectionCount(value: unknown): number {
  const n = Number(value);
  if (Number.isInteger(n) && n > 0 && PLATFORM_CAPS.scanSectionCounts.includes(n)) return n;
  return 1;
}

export function isValidSectionValue(value: unknown): boolean {
  const n = Number(value);
  return Number.isInteger(n) && PLATFORM_CAPS.scanSectionCounts.includes(n);
}
