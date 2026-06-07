import { getBehavior } from "@cellsymphony/behavior-api";
import { interpretGrid, type AxisStrategy, type InterpretationProfile, type TickStrategy } from "@cellsymphony/interpretation-core";
import { mapIntentsToMusicalEvents } from "@cellsymphony/mapping-core";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { applyModulationResult, applyNoteBehavior, withScaleSteps } from "./musicTransforms";
import { dedupeSimultaneousNotes, filterTriggerGatedIntents, toGridSnapshot } from "./runtimeHelpers";
import { mod, mergeMapping } from "./coreUtils";
import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { PlatformState, RuntimeConfig, Direction, NoteUnit } from "./platformTypes";
import { clampPartIndex, PLATFORM_CAPS, sectionCount } from "./platformCaps";
import { TRANSPORT_FLASH_MS, deadlineMs, nowMs } from "./timing";
import { resetScanState } from "./transportSafety";

const PPQN = 24;

export function tickTransport<TState>(state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, elapsedSeconds: number): { state: PlatformState<TState>; events: MusicalEvent[] } {
  const events: MusicalEvent[] = [];
  let next = { ...state };
  const prevPulse = next.transport.ppqnPulse;
  if (next.runtimeConfig.midi.syncMode === "external") return { state: next, events };
  if (!next.transport.playing) return { state: next, events };

  const elapsedPulses = pulsesPerSecond(next.transport.bpm) * elapsedSeconds;
  next.partScanPulseAccumulator = next.partScanPulseAccumulator.map((v) => v + elapsedPulses);
  next.partAlgorithmPulseAccumulator = next.partAlgorithmPulseAccumulator.map((v) => v + elapsedPulses);
  next.scanPulseAccumulator = next.partScanPulseAccumulator[clampPartIndex((next.runtimeConfig as any).activePartIndex ?? 0)] ?? next.scanPulseAccumulator;
  next.algorithmPulseAccumulator = next.partAlgorithmPulseAccumulator[clampPartIndex((next.runtimeConfig as any).activePartIndex ?? 0)] ?? next.algorithmPulseAccumulator;
  next.ppqnPulseRemainder += elapsedPulses;
  const wholePulses = Math.floor(next.ppqnPulseRemainder);
  if (wholePulses > 0) {
    next.ppqnPulseRemainder -= wholePulses;
    next.transport = { ...next.transport, ppqnPulse: next.transport.ppqnPulse + wholePulses };
  }

  const advanced = advanceEngineByPulses(next, behavior, 0);
  next = advanced.state;
  events.push(...advanced.events);
  next = applyBeatFlash(next, prevPulse);
  return { state: next, events };
}

export function applyExternalClockPulses<TState>(state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, pulses: number): { state: PlatformState<TState>; events: MusicalEvent[] } {
  const events: MusicalEvent[] = [];
  if (pulses <= 0) return { state, events };
  let next = { ...state };
  const prevExt = next.system.externalPpqnPulse;
  const nextExt = prevExt + pulses;
  next.system = { ...next.system, externalPpqnPulse: nextExt };

  if (next.system.pendingResync) {
    const target = prevExt + (96 - (prevExt % 96 || 96));
    if (nextExt >= target) {
      const scanReset = resetScanState(next);
      next.transport = { ...next.transport, ppqnPulse: target, tick: 0 };
      next.partScanPulseAccumulator = scanReset.partScanPulseAccumulator;
      next.partAlgorithmPulseAccumulator = scanReset.partAlgorithmPulseAccumulator;
      next.ppqnPulseRemainder = scanReset.ppqnPulseRemainder;
      next.partScanIndex = scanReset.partScanIndex;
      next.scanIndex = scanReset.scanIndex;
      next.scanPulseAccumulator = scanReset.scanPulseAccumulator;
      next.algorithmPulseAccumulator = scanReset.algorithmPulseAccumulator;
      next.system = { ...next.system, pendingResync: false };
    }
  }

  if (!next.transport.playing) return { state: next, events };
  return advanceEngineByPulses(next, behavior, pulses);
}

function advanceEngineByPulses<TState>(state: PlatformState<TState>, behavior: BehaviorEngine<TState, unknown>, pulses: number): { state: PlatformState<TState>; events: MusicalEvent[] } {
  const events: MusicalEvent[] = [];
  let next = { ...state };
  const activePart = clampPartIndex((next.runtimeConfig as any).activePartIndex ?? 0);
  const parts: any[] = Array.isArray((next.runtimeConfig as any).parts) ? (next.runtimeConfig as any).parts : [];
  let partStates: any[] = Array.isArray((next as any).partStates) ? ([...(next as any).partStates] as any[]) : [];
  while (partStates.length < PLATFORM_CAPS.partCount) partStates.push(next.behaviorState);
  let partScanIndex = Array.isArray((next as any).partScanIndex) ? ([...(next as any).partScanIndex] as number[]) : Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  let partScanPulseAccumulator = Array.isArray((next as any).partScanPulseAccumulator) ? ([...(next as any).partScanPulseAccumulator] as number[]) : Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  let partAlgorithmPulseAccumulator = Array.isArray((next as any).partAlgorithmPulseAccumulator) ? ([...(next as any).partAlgorithmPulseAccumulator] as number[]) : Array.from({ length: PLATFORM_CAPS.partCount }, () => 0);
  if (pulses > 0) {
    partScanPulseAccumulator = partScanPulseAccumulator.map((v) => v + pulses);
    partAlgorithmPulseAccumulator = partAlgorithmPulseAccumulator.map((v) => v + pulses);
    next.transport = { ...next.transport, ppqnPulse: next.transport.ppqnPulse + pulses };
  }

  let heldNotes = next.system.heldNotes;
  for (let partIdx = 0; partIdx < PLATFORM_CAPS.partCount; partIdx += 1) {
    const part = parts[partIdx];
    if (!part) continue;
    const partCfg = toRuntimeConfigForPart(next.runtimeConfig, next.mappingConfig as any, part, behavior.id, partIdx === activePart);
    const engine = (getBehavior(partCfg.activeBehavior) as BehaviorEngine<any, unknown> | undefined) ?? behavior;
    const beforeGrid = toGridSnapshot(engine.renderModel(partStates[partIdx]));

    let scanAdvanced = false;
    if (partCfg.scanMode === "scanning") {
      const scanStepPulses = noteUnitToPulses(partCfg.scanUnit);
      while (partScanPulseAccumulator[partIdx] >= scanStepPulses) {
        partScanPulseAccumulator[partIdx] -= scanStepPulses;
        scanAdvanced = true;
      }
    }

    const algorithmStepPulses = noteUnitToPulses(partCfg.algorithmStepUnit);
    while (partAlgorithmPulseAccumulator[partIdx] >= algorithmStepPulses) {
      partAlgorithmPulseAccumulator[partIdx] -= algorithmStepPulses;
      partStates[partIdx] = engine.onTick(partStates[partIdx], { bpm: next.transport.bpm, emit: () => {} });
    }
    const afterGrid = toGridSnapshot(engine.renderModel(partStates[partIdx]));
    const shouldInterpret =
      partCfg.scanMode === "immediate" ||
      partCfg.eventEnabled ||
      scanAdvanced;
    if (!shouldInterpret) continue;
    const profile = profileFromConfig(partCfg);
    const interpretationTick = partCfg.scanMode === "scanning" ? partScanIndex[partIdx] : next.transport.tick;
    const intents = interpretGrid(beforeGrid, afterGrid, interpretationTick, profile);
    const gated = filterTriggerGatedIntents(intents, next, partIdx);
    const mapped = mapIntentsToMusicalEvents(gated, withScaleSteps(partCfg.mappingConfig as any, partCfg));
    const modulation = applyModulationResult(gated, mapped, partCfg, next.runtimeConfig, partIdx);
    next.runtimeConfig = modulation.runtimeConfig;
    const modulated = modulation.events;
    const instruments: any[] = Array.isArray((next.runtimeConfig as any).instruments) ? ((next.runtimeConfig as any).instruments as any[]) : [];
    const shaped = applyNoteBehavior(modulated, instruments, partIdx, heldNotes);
    heldNotes = shaped.heldNotes;
    events.push(...shaped.events);
    if (scanAdvanced && partCfg.scanMode === "scanning") {
      partScanIndex[partIdx] = advanceScanIndex(partScanIndex[partIdx], partCfg.scanDirection, scanIndexSpan(partCfg));
    }
  }

  next.system = { ...next.system, heldNotes };
  next.partStates = partStates;
  next.partScanIndex = partScanIndex;
  next.partScanPulseAccumulator = partScanPulseAccumulator;
  next.partAlgorithmPulseAccumulator = partAlgorithmPulseAccumulator;
  next.behaviorState = partStates[activePart] as TState;
  next.scanIndex = partScanIndex[activePart] ?? next.scanIndex;
  next.scanPulseAccumulator = partScanPulseAccumulator[activePart] ?? next.scanPulseAccumulator;
  next.algorithmPulseAccumulator = partAlgorithmPulseAccumulator[activePart] ?? next.algorithmPulseAccumulator;

  next.transport = { ...next.transport, tick: next.transport.tick + 1 };
  return { state: next, events: dedupeSimultaneousNotes(events) };
}

function applyBeatFlash<TState>(state: PlatformState<TState>, prevPulse: number): PlatformState<TState> {
  let next = { ...state };
  if (next.transport.playing && next.transport.ppqnPulse > prevPulse) {
    let sawBeat = false;
    let sawMeasure = false;
    for (let pulse = prevPulse + 1; pulse <= next.transport.ppqnPulse; pulse += 1) {
      if (pulse % 96 === 0) sawMeasure = true;
      else if (pulse % 24 === 0) sawBeat = true;
    }
    const now = nowMs();
    if (sawMeasure) next.system = { ...next.system, transportFlash: "measure", transportFlashUntilMs: deadlineMs(now, TRANSPORT_FLASH_MS) };
    else if (sawBeat) next.system = { ...next.system, transportFlash: "beat", transportFlashUntilMs: deadlineMs(now, TRANSPORT_FLASH_MS) };
  }
  return next;
}

function pulsesPerSecond(bpm: number): number {
  return (bpm / 60) * PPQN;
}

function noteUnitToPulses(unit: NoteUnit): number {
  switch (unit) {
    case "1/16":
      return 6;
    case "1/8":
      return 12;
    case "1/4":
      return 24;
    case "1/2":
      return 48;
    case "1/1":
      return 96;
  }
  return 24;
}

function advanceScanIndex(current: number, direction: Direction, size: number): number {
  const delta = direction === "reverse" ? -1 : 1;
  return mod(current + delta, size);
}

function scanIndexSpan(cfg: RuntimeConfig): number {
  const sections = sectionCount(cfg.scanSections);
  if (sections <= 1) return cfg.scanAxis === "columns" ? PLATFORM_CAPS.gridWidth : PLATFORM_CAPS.gridHeight;
  return cfg.scanAxis === "columns" ? PLATFORM_CAPS.gridHeight * sections : PLATFORM_CAPS.gridWidth * sections;
}

function profileFromConfig(cfg: RuntimeConfig): InterpretationProfile {
  const tick: TickStrategy = cfg.scanMode === "immediate"
    ? { mode: "whole_grid_transitions" }
    : { mode: cfg.scanAxis === "columns" ? "scan_column_active" : "scan_row_active", sections: sectionCount(cfg.scanSections) };
  const axisX: AxisStrategy = cfg.x.pitch.enabled ? { mode: "scale_step", step: Math.abs(cfg.x.pitch.steps) } : { mode: "timing_only" };
  const axisY: AxisStrategy = cfg.y.pitch.enabled ? { mode: "scale_step", step: Math.abs(cfg.y.pitch.steps) } : { mode: "timing_only" };
  return {
    id: "menu_profile",
    event: { enabled: cfg.eventEnabled },
    state: { enabled: true, tick },
    x: axisX,
    y: axisY
  };
}

function toRuntimeConfigForPart(base: RuntimeConfig, mapping: any, part: any, fallbackBehaviorId: string, preferBase: boolean): RuntimeConfig & { mappingConfig: any } {
  const behaviorId = String(part?.l1?.behaviorId ?? fallbackBehaviorId);
  const partCfg = {
    ...base,
    algorithmStepUnit: preferBase ? base.algorithmStepUnit : (part?.l1?.stepRate ?? base.algorithmStepUnit),
    activeBehavior: preferBase ? base.activeBehavior : behaviorId,
    behaviorConfig: {
      ...(base.behaviorConfig as any),
      [behaviorId]: { ...((preferBase ? (base.behaviorConfig as any)?.[behaviorId] : undefined) ?? part?.l1?.behaviorConfig ?? {}) }
    },
    scanMode: preferBase ? base.scanMode : (part?.l2?.scanMode ?? base.scanMode),
    scanAxis: preferBase ? base.scanAxis : (part?.l2?.scanAxis ?? base.scanAxis),
    scanUnit: preferBase ? base.scanUnit : (part?.l2?.scanUnit ?? base.scanUnit),
    scanDirection: preferBase ? base.scanDirection : (part?.l2?.scanDirection ?? base.scanDirection),
    scanSections: preferBase ? base.scanSections : (part?.l2?.scanSections ?? base.scanSections),
    eventEnabled: preferBase ? base.eventEnabled : (part?.l2?.eventEnabled ?? base.eventEnabled),
    pitch: preferBase ? base.pitch : (part?.l2?.pitch ? structuredClone(part.l2.pitch) : base.pitch),
    x: preferBase ? base.x : (part?.l2?.x ? structuredClone(part.l2.x) : base.x),
    y: preferBase ? base.y : (part?.l2?.y ? structuredClone(part.l2.y) : base.y)
  } as RuntimeConfig;
  const mappingConfig = mergeMapping(mapping, part, preferBase);
  return { ...partCfg, mappingConfig };
}
