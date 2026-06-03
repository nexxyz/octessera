import type { RuntimeConfig } from "./platformTypes";
import { getMappableParams } from "./menuParamList";
import { applyParamModulation } from "./musicTransforms";

describe("ParamModConfig", () => {
  test("initializes with two empty slots", () => {
    const config = { slots: [
      { key: "", label: "", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "", label: "", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }
    ]};
    expect(config.slots.length).toBe(2);
    expect(config.slots[0].key).toBe("");
    expect(config.slots[1].key).toBe("");
  });

  test("stores parameter key and label", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }
    ]};
    expect(config.slots[0].key).toBe("filterCutoff");
    expect(config.slots[0].label).toBe("Filter Cutoff");
    expect(config.slots[1].key).toBe("filterResonance");
    expect(config.slots[1].label).toBe("Filter Resonance");
  });

  test("stores mapping coordinates", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 2, y: 3, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 4, y: 5, mode: "modulation" }, invert: false }
    ]};
    expect(config.slots[0].mapping.x).toBe(2);
    expect(config.slots[0].mapping.y).toBe(3);
    expect(config.slots[1].mapping.x).toBe(4);
    expect(config.slots[1].mapping.y).toBe(5);
  });

  test("stores invert flag", () => {
    const config = { slots: [
      { key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false },
      { key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: true }
    ]};
    expect(config.slots[0].invert).toBe(false);
    expect(config.slots[1].invert).toBe(true);
  });
});

describe("applyModulation", () => {
  test("applies modulation from Slot 1", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 0, y: 0 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("note_on");
    expect(result[0].x).toBe(0);
    expect(result[0].y).toBe(0);
  });

  test("applies modulation from Slot 2", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 1, y: 1 }];
    const intents = [{ x: 1, y: 1, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("note_on");
    expect(result[0].x).toBe(1);
    expect(result[0].y).toBe(1);
  });

  test("handles invert mode", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: true }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 0, y: 0 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("note_on");
    expect(result[0].x).toBe(0);
    expect(result[0].y).toBe(0);
  });

  test("ignores unmapped slots", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 3, y: 3 }];
    const intents = [{ x: 3, y: 3, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("note_on");
    expect(result[0].x).toBe(3);
    expect(result[0].y).toBe(3);
  });
});

describe("handleParamModAssign", () => {
  test("assigns parameter to X axis", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 2, y: 0 }];
    const intents = [{ x: 2, y: 0, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("cc");
    expect(result[0].channel).toBe(0);
    expect(result[0].controller).toBe(74);
    expect(result[0].value).toBeGreaterThan(0);
  });

  test("assigns parameter to Y axis", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterResonance", label: "Filter Resonance", kind: "number", mapping: { x: 1, y: 1, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 1, y: 2 }];
    const intents = [{ x: 1, y: 2, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("cc");
    expect(result[0].channel).toBe(0);
    expect(result[0].controller).toBe(71);
    expect(result[0].value).toBeGreaterThan(0);
  });

  test("assigns parameter to both axes (0,0 or 1,1)", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 0, y: 0 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("cc");
    expect(result[0].channel).toBe(0);
    expect(result[0].controller).toBe(74);
    expect(result[0].value).toBe(0);
  });

  test("toggles invert mode", () => {
    const cfg: RuntimeConfig = { parts: [{ l2: {}, modSlots: { slots: [{ key: "filterCutoff", label: "Filter Cutoff", kind: "number", mapping: { x: 0, y: 0, mode: "modulation" }, invert: false }] } ] };
    const events = [{ type: "note_on", channel: 0, note: 60, velocity: 64, x: 0, y: 0 }];
    const intents = [{ x: 0, y: 0, degree: 0, kind: "activate" }];
    const result = applyParamModulation(events, intents, cfg);
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("cc");
    expect(result[0].channel).toBe(0);
    expect(result[0].controller).toBe(74);
    expect(result[0].value).toBe(0);
  });
});

describe("getMappableParams", () => {
  test("returns list of mappable parameters", () => {
    const params = getMappableParams();
    expect(params.length).toBeGreaterThan(0);
  });

  test("includes filterCutoff parameter", () => {
    const params = getMappableParams();
    const cutoff = params.find((p) => p.key === "filterCutoff");
    expect(cutoff).toBeDefined();
    expect(cutoff?.key).toBe("filterCutoff");
    expect(cutoff?.label).toBe("Filter Cutoff");
    expect(cutoff?.kind).toBe("number");
    expect(cutoff?.min).toBe(20);
    expect(cutoff?.max).toBe(20000);
    expect(cutoff?.step).toBe(1);
  });

  test("includes filterResonance parameter", () => {
    const params = getMappableParams();
    const resonance = params.find((p) => p.key === "filterResonance");
    expect(resonance).toBeDefined();
    expect(resonance?.key).toBe("filterResonance");
    expect(resonance?.label).toBe("Filter Resonance");
    expect(resonance?.kind).toBe("number");
    expect(resonance?.min).toBe(0);
    expect(resonance?.max).toBe(20);
    expect(resonance?.step).toBe(0.1);
  });

  test("includes velocityScalePct parameter", () => {
    const params = getMappableParams();
    const velocity = params.find((p) => p.key === "velocityScalePct");
    expect(velocity).toBeDefined();
    expect(velocity?.key).toBe("velocityScalePct");
    expect(velocity?.label).toBe("Velocity Scale %");
  });

  test("includes gainPct parameter", () => {
    const params = getMappableParams();
    const gain = params.find((p) => p.key === "gainPct");
    expect(gain).toBeDefined();
    expect(gain?.key).toBe("gainPct");
    expect(gain?.label).toBe("Gain %");
  });

  test("includes noteLengthMs parameter", () => {
    const params = getMappableParams();
    const noteLen = params.find((p) => p.key === "noteLengthMs");
    expect(noteLen).toBeDefined();
    expect(noteLen?.key).toBe("noteLengthMs");
    expect(noteLen?.label).toBe("Note Length ms");
  });
});