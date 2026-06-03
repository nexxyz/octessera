import type { RuntimeConfig } from "./platformTypes";
import { getMappableParams } from "./menuParamList";
import { applyModulation } from "./musicTransforms";

declare function describe(name: string, fn: () => void): void;
declare function test(name: string, fn: () => void): void;

function eq<T>(actual: T, expected: T): void {
  if (actual !== expected) throw new Error(`Expected ${expected}, got ${actual}`);
}
function ok(condition: boolean): void {
  if (!condition) throw new Error("Assertion failed");
}

describe("ParamModConfig", () => {
  test("initializes with two empty slots", () => {
    const config = { slots: [
      { key: "", label: "", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "", label: "", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }
    ]};
    eq(config.slots.length, 2);
    eq(config.slots[0].key, "");
    eq(config.slots[1].key, "");
  });

  test("stores parameter key and label", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }
    ]};
    eq(config.slots[0].key, "filterCutoff");
    eq(config.slots[0].label, "Filter Cutoff");
    eq(config.slots[1].key, "filterResonance");
    eq(config.slots[1].label, "Filter Resonance");
  });

  test("stores mapping coordinates", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 2, y: 3, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 4, y: 5, mode: "modulation" }, invert: false }
    ]};
    eq(config.slots[0].mapping.x, 2);
    eq(config.slots[0].mapping.y, 3);
    eq(config.slots[1].mapping.x, 4);
    eq(config.slots[1].mapping.y, 5);
  });

  test("stores invert flag", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: true }
    ]};
    eq(config.slots[0].invert, false);
    eq(config.slots[1].invert, true);
  });
});

describe("applyModulation", () => {
  function modCfg(x: number, y: number, invert?: boolean) {
    return {
      parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x, y, mode: "modulation" }, invert: invert ?? false }] } }]
    } as unknown as RuntimeConfig;
  }

  test("applies modulation from Slot 1", () => {
    const cfg = modCfg(0, 0);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    eq(result[0].type, "note_on");
  });

  test("applies modulation from Slot 2", () => {
    const cfg = modCfg(1, 1);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 1, y: 1, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    eq(result[0].type, "note_on");
  });

  test("handles invert mode", () => {
    const cfg = modCfg(0, 0, true);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    eq(result[0].type, "note_on");
  });

  test("ignores unmapped slots", () => {
    const cfg = modCfg(0, 0);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 3, y: 3, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    eq(result[0].type, "note_on");
  });
});

describe("handleParamModAssign", () => {
  function modCfg2(key: string, x: number, y: number) {
    return {
      parts: [{ l2: {}, modSlots: { slots: [{ key, label: key === "filterCutoff" ? "Filter Cutoff" : "Filter Resonance", kind: "number", mapping: { x, y, mode: "modulation" }, invert: false }] } }]
    } as unknown as RuntimeConfig;
  }

  test("assigns parameter to X axis", () => {
    const cfg = modCfg2("filterCutoff", 0, 0);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 2, y: 0, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    const cc = result[0];
    if (cc.type === "cc") {
      eq(cc.channel, 0);
      eq(cc.controller, 74);
      ok(cc.value > 0);
    }
  });

  test("assigns parameter to Y axis", () => {
    const cfg = modCfg2("filterResonance", 1, 1);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 1, y: 2, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    const cc = result[0];
    if (cc.type === "cc") {
      eq(cc.channel, 0);
      eq(cc.controller, 71);
      ok(cc.value > 0);
    }
  });

  test("assigns parameter to both axes (0,0 or 1,1)", () => {
    const cfg = modCfg2("filterCutoff", 0, 0);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    const cc = result[0];
    if (cc.type === "cc") {
      eq(cc.channel, 0);
      eq(cc.controller, 74);
      eq(cc.value, 0);
    }
  });

  test("toggles invert mode", () => {
    const cfg = modCfg2("filterCutoff", 0, 0);
    const events = [{ type: "note_on" as const, channel: 0, note: 60, velocity: 64 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyModulation(intents, events, cfg);
    eq(result.length, 1);
    const cc = result[0];
    if (cc.type === "cc") {
      eq(cc.channel, 0);
      eq(cc.controller, 74);
      eq(cc.value, 0);
    }
  });
});

describe("getMappableParams", () => {
  test("returns list of mappable parameters", () => {
    const params = getMappableParams();
    ok(params.length > 0);
  });

  test("includes filterCutoff parameter", () => {
    const params = getMappableParams();
    const cutoff = params.find((p) => p.key === "filterCutoff");
    ok(cutoff !== undefined);
    eq(cutoff!.key, "filterCutoff");
    eq(cutoff!.label, "Filter Cutoff");
    eq(cutoff!.kind, "number");
    eq(cutoff!.min, 20);
    eq(cutoff!.max, 20000);
    eq(cutoff!.step, 1);
  });

  test("includes filterResonance parameter", () => {
    const params = getMappableParams();
    const resonance = params.find((p) => p.key === "filterResonance");
    ok(resonance !== undefined);
    eq(resonance!.key, "filterResonance");
    eq(resonance!.label, "Filter Resonance");
    eq(resonance!.kind, "number");
    eq(resonance!.min, 0);
    eq(resonance!.max, 20);
    eq(resonance!.step, 0.1);
  });

  test("includes velocityScalePct parameter", () => {
    const params = getMappableParams();
    const velocity = params.find((p) => p.key === "velocityScalePct");
    ok(velocity !== undefined);
    eq(velocity!.key, "velocityScalePct");
    eq(velocity!.label, "Velocity Scale %");
  });

  test("includes gainPct parameter", () => {
    const params = getMappableParams();
    const gain = params.find((p) => p.key === "gainPct");
    ok(gain !== undefined);
    eq(gain!.key, "gainPct");
    eq(gain!.label, "Gain %");
  });

  test("includes noteLengthMs parameter", () => {
    const params = getMappableParams();
    const noteLen = params.find((p) => p.key === "noteLengthMs");
    ok(noteLen !== undefined);
    eq(noteLen!.key, "noteLengthMs");
    eq(noteLen!.label, "Note Length ms");
  });
});
