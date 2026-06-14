import type { MusicalEvent } from "@cellsymphony/musical-events";
import type { PlatformState } from "./platformTypes";
import { PLATFORM_CAPS, clampPartIndex, sectionCount } from "./platformCaps";

export function resetScanState<TState>(state: PlatformState<TState>): Pick<PlatformState<TState>, "scanIndex" | "scanPulseAccumulator" | "algorithmPulseAccumulator" | "ppqnPulseRemainder" | "partScanIndex" | "partScanPulseAccumulator" | "partAlgorithmPulseAccumulator"> {
  const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
  const partScanIndex = Array.from({ length: PLATFORM_CAPS.partCount }, (_, partIdx) => originIndex(state.runtimeConfig as any, partIdx, activePart));
  return {
    scanIndex: originIndex(state.runtimeConfig as any, activePart, activePart),
    scanPulseAccumulator: 0,
    algorithmPulseAccumulator: 0,
    ppqnPulseRemainder: 0,
    partScanIndex,
    partScanPulseAccumulator: Array.from({ length: PLATFORM_CAPS.partCount }, () => 0),
    partAlgorithmPulseAccumulator: Array.from({ length: PLATFORM_CAPS.partCount }, () => 0)
  };
}

export function emergencyBrakeState<TState>(state: PlatformState<TState>): { state: PlatformState<TState>; events: MusicalEvent[] } {
  const scanReset = resetScanState(state);
  const events: MusicalEvent[] = [];
  for (let channel = 0; channel < 16; channel += 1) {
    events.push({ type: "cc", channel, controller: 120, value: 0 });
    events.push({ type: "cc", channel, controller: 123, value: 0 });
  }
  return {
    state: {
      ...state,
      transport: { ...state.transport, playing: false, ppqnPulse: 0 },
      system: { ...state.system, stopLatched: true, transportFlash: "none", transportFlashUntilMs: 0, heldNotes: [] },
      scanIndex: scanReset.scanIndex,
      scanPulseAccumulator: scanReset.scanPulseAccumulator,
      algorithmPulseAccumulator: scanReset.algorithmPulseAccumulator,
      ppqnPulseRemainder: scanReset.ppqnPulseRemainder,
      partScanIndex: scanReset.partScanIndex,
      partScanPulseAccumulator: scanReset.partScanPulseAccumulator,
      partAlgorithmPulseAccumulator: scanReset.partAlgorithmPulseAccumulator
    },
    events
  };
}

function effectiveScanConfig(runtimeConfig: any, partIdx: number, activePart: number): { scanAxis: string; scanDirection: string; scanSections: unknown } {
  if (partIdx === activePart) {
    return {
      scanAxis: runtimeConfig.scanAxis,
      scanDirection: runtimeConfig.scanDirection,
      scanSections: runtimeConfig.scanSections
    };
  }
  const partL2 = runtimeConfig.parts?.[partIdx]?.l2;
  return {
    scanAxis: partL2?.scanAxis ?? runtimeConfig.scanAxis,
    scanDirection: partL2?.scanDirection ?? runtimeConfig.scanDirection,
    scanSections: partL2?.scanSections ?? runtimeConfig.scanSections
  };
}

function originIndex(runtimeConfig: any, partIdx: number, activePart: number): number {
  const { scanDirection } = effectiveScanConfig(runtimeConfig, partIdx, activePart);
  const size = scanIndexSpan(runtimeConfig, partIdx, activePart);
  return scanDirection === "forward" ? 0 : size - 1;
}

function scanIndexSpan(runtimeConfig: any, partIdx: number, activePart: number): number {
  const { scanAxis, scanSections } = effectiveScanConfig(runtimeConfig, partIdx, activePart);
  const sections = sectionCount(scanSections);
  if (sections <= 1) return scanAxis === "columns" ? PLATFORM_CAPS.gridWidth : PLATFORM_CAPS.gridHeight;
  return scanAxis === "columns" ? PLATFORM_CAPS.gridWidth * sections : PLATFORM_CAPS.gridHeight * sections;
}
