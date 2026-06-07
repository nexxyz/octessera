import { getBehavior, type BehaviorEngine } from "@cellsymphony/behavior-api";
import type { MappingConfig } from "@cellsymphony/mapping-core";
import type { ConfigPayload, MenuNode, PlatformState } from "./platformTypes";
import { clampPartIndex, PLATFORM_CAPS } from "./platformCaps";
import { SYNTH_PRESETS } from "./synthPresets";
import { clamp, mod } from "./coreUtils";
import { readAnyValue } from "./paramAccess";
import { createDefaultTriggerProbabilityMap } from "./triggerProbability";

export function textEditTurn<TState>(state: PlatformState<TState>, node: Extract<MenuNode, { kind: "text" }>, delta: -1 | 1): PlatformState<TState> {
  const raw = String(readAnyValue(state, node.key) ?? "");
  const cursor = clamp(state.system.nameCursor, 0, Math.max(0, node.maxLen));
  const safe = raw.slice(0, node.maxLen);
  const curPos = clamp(cursor, 0, safe.length);
  const charset = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
  const chars = safe.split("");
  while (chars.length <= curPos) chars.push(" ");
  const current = chars[curPos] ?? " ";
  const idx = Math.max(0, charset.indexOf(current));
  const nextIdx = mod(idx + delta, charset.length);
  chars[curPos] = charset[nextIdx] ?? " ";
  const next = chars.join("").replace(/\s+$/g, "");
  return {
    ...state,
    system: { ...state.system, draftName: next, nameCursor: curPos }
  };
}

export function formatTimestamp(nowMs: number): string {
  const d = new Date(nowMs);
  const yyyy = d.getFullYear();
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  const dd = String(d.getDate()).padStart(2, "0");
  const hh = String(d.getHours()).padStart(2, "0");
  const min = String(d.getMinutes()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd} ${hh}${min}`;
}

export function factoryPayload<TState>(behavior: BehaviorEngine<TState, unknown>, createInitialState: (b: BehaviorEngine<TState, unknown>) => PlatformState<TState>, extractConfigPayload: (s: PlatformState<TState>) => ConfigPayload): ConfigPayload {
  const s = createInitialState(behavior);
  const rc: any = s.runtimeConfig;
  const parts: any[] = rc.parts;
  const instruments: any[] = rc.instruments;

  parts[0].l1.behaviorId = "life";
  parts[0].l1.behaviorConfig = { ...((rc.behaviorConfig ?? {}).life ?? {}), randomCellsPerTick: 12, randomTickInterval: 1 };
  parts[0].l1.stepRate = "1/8";
  parts[0].l2.mapping = {
    activate: { action: "note_on", slot: 0 },
    stable: { action: "none", slot: 0 },
    deactivate: { action: "note_off", slot: 0 },
    scanned: { action: "none", slot: 0 },
    scanned_empty: { action: "note_off", slot: 0 }
  };
  parts[0].l2.scanAxis = "columns";
  parts[0].l2.eventEnabled = true;
  parts[0].name = "life";
  parts[0].autoName = true;
  parts[0].l2.triggerProbabilityMode = "full";
  parts[0].l2.triggerProbabilityMap = createDefaultTriggerProbabilityMap();

  parts[1].l1.behaviorId = "sequencer";
  parts[1].l1.behaviorConfig = {};
  parts[1].l1.stepRate = "1/4";
  parts[1].l2.mapping = {
    activate: { action: "none", slot: 0 },
    stable: { action: "none", slot: 0 },
    deactivate: { action: "none", slot: 0 },
    scanned: { action: "note_on", slot: 1 },
    scanned_empty: { action: "note_off", slot: 1 }
  };
  parts[1].l2.scanAxis = "rows";
  parts[1].l2.eventEnabled = true;
  parts[1].name = "sequencer";
  parts[1].autoName = true;
  parts[1].l2.triggerProbabilityMode = "full";
  parts[1].l2.triggerProbabilityMap = createDefaultTriggerProbabilityMap();

  for (let i = 2; i < PLATFORM_CAPS.partCount; i += 1) {
    parts[i].l1.behaviorId = "none";
    parts[i].l1.behaviorConfig = {};
    parts[i].l2.mapping = {
      activate: { action: "none", slot: 0 },
      stable: { action: "none", slot: 0 },
      deactivate: { action: "none", slot: 0 },
      scanned: { action: "none", slot: 0 },
      scanned_empty: { action: "none", slot: 0 }
    };
    parts[i].l2.eventEnabled = false;
    parts[i].name = "(none)";
    parts[i].autoName = true;
    parts[i].l2.triggerProbabilityMode = "full";
    parts[i].l2.triggerProbabilityMap = createDefaultTriggerProbabilityMap();
  }

  instruments[0].type = "synth";
  instruments[0].synth = structuredClone(SYNTH_PRESETS[1]!.synth);
  instruments[0].mixer = { route: "fx_bus_1", panPos: 0, volume: 100 };
  instruments[0].name = "synth";
  instruments[0].autoName = true;
  instruments[0].noteBehavior = "oneshot";

  instruments[1].type = "synth";
  instruments[1].synth = structuredClone(SYNTH_PRESETS[7]!.synth);
  instruments[1].mixer = { route: "direct", panPos: 0, volume: 100 };
  instruments[1].name = "drums";
  instruments[1].autoName = true;
  instruments[1].noteBehavior = "oneshot";

  for (let i = 2; i < PLATFORM_CAPS.instrumentCount; i += 1) {
    instruments[i].type = "none";
    instruments[i].mixer = { route: "direct", panPos: 0, volume: 100 };
    instruments[i].name = "(none)";
    instruments[i].autoName = true;
  }

  rc.mixer.buses[0] = {
    slot1: { type: "delay", params: { timeMs: 280, feedbackPct: 38, mixPct: 45 } },
    slot2: { type: "duck", params: { source: "I2", threshold: 0.08, amountPct: 60, attackMs: 8, releaseMs: 160 } },
    panPos: 0,
    autoName: true,
    name: "Bus 1"
  };

  for (let i = 1; i < PLATFORM_CAPS.busCount; i += 1) {
    rc.mixer.buses[i] = {
      slot1: { type: "none", params: {} },
      slot2: { type: "none", params: {} },
      panPos: 0,
      autoName: true,
      name: "(none)"
    };
  }

  rc.activePartIndex = 0;
  rc.activeBehavior = "life";

  const partStates: any[] = [];
  for (const p of parts) {
    const engine = getBehavior(p.l1.behaviorId) ?? behavior;
    partStates.push(engine.init({ ...(p.l1.behaviorConfig ?? {}) }));
  }
  s.partStates = partStates;
  s.behaviorState = partStates[0];
  s.activeBehavior = "life";

  return extractConfigPayload({ ...s, runtimeConfig: rc });
}
