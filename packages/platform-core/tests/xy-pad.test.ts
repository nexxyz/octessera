import test from "node:test";
import assert from "node:assert/strict";
import type { RuntimeConfig } from "../src/platformTypes";
import { applyXyModulation } from "../src/musicTransforms";

test("applyXyModulation applies number binding from X axis", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null } }],
    xyTouch: { x: 0.5, y: 0.5, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 50);
});

test("applyXyModulation applies number binding from Y axis", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: null, y: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 } } }],
    xyTouch: { x: 0.5, y: 0.75, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 75);
});

test("applyXyModulation applies enum binding", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "scanUnit", kind: "enum", options: ["1/16", "1/8", "1/4", "1/2", "1/1"] }, y: null } }],
    xyTouch: { x: 0.3, y: 0, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).scanUnit, "1/8");
});

test("applyXyModulation applies bool binding", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "eventEnabled", kind: "bool" }, y: null } }],
    xyTouch: { x: 0.7, y: 0, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).eventEnabled, true);
});

test("applyXyModulation returns unmodified cfg when touch not active and release is reset-center", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null } }],
    xyTouch: { x: 0.5, y: 0.5, active: false },
    xyRelease: "reset-center"
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 73);
});

test("applyXyModulation keeps modulation when release is sample-hold and touch inactive", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null } }],
    xyTouch: { x: 0.7, y: 0.5, active: false },
    xyRelease: "sample-hold"
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 70);
});

test("applyXyModulation returns unmodified cfg when no xy targets are assigned", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: null, y: null } }],
    xyTouch: { x: 0.5, y: 0.5, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 73);
});

test("applyXyModulation uses activePartIndex from cfg when partIndex not provided", () => {
  const cfg = makeCfg({
    parts: [
      { xy: { x: null, y: null } },
      { xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null } }
    ],
    xyTouch: { x: 0.3, y: 0.5, active: true },
    activePartIndex: 1
  });
  const result = applyXyModulation(cfg);
  assert.equal((result as any).masterVolume, 30);
});

test("applyXyModulation inverts X axis when xInvert is true", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null, xInvert: true } }],
    xyTouch: { x: 0.3, y: 0.5, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 70);
});

test("applyXyModulation inverts Y axis when yInvert is true", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: null, y: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, yInvert: true } }],
    xyTouch: { x: 0.5, y: 0.2, active: true }
  });
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 80);
});

test("applyXyModulation does not touch unbound axis", () => {
  const cfg = makeCfg({
    parts: [{ xy: { x: { key: "masterVolume", kind: "number", min: 0, max: 100, step: 1 }, y: null } }],
    xyTouch: { x: 0.5, y: 0.5, active: true }
  });
  const prev = (cfg as any).scanMode;
  const result = applyXyModulation(cfg, 0);
  assert.equal((result as any).masterVolume, 50);
  assert.equal((result as any).scanMode, prev);
});

function makeCfg(overrides?: Partial<RuntimeConfig>): RuntimeConfig {
  return {
    activeBehavior: "life",
    scanMode: "none",
    scanAxis: "columns",
    scanUnit: "1/16",
    scanDirection: "forward",
    scanSections: "1",
    eventEnabled: true,
    ghostCells: true,
    autoSaveDefault: false,
    screenSleepSeconds: 60,
    displayBrightness: 75,
    gridBrightness: 75,
    buttonBrightness: 75,
    numericDisplayMode: "bar+numbers",
    masterVolume: 73,
    sound: {
      noteLengthMs: 120,
      velocityScalePct: 100,
      velocityCurve: "linear",
      voiceStealingMode: "balanced"
    },
    pitch: {
      startingNote: 60,
      lowestNote: 24,
      highestNote: 96,
      scale: "chromatic",
      root: "C",
      outOfRange: "clamp"
    },
    instruments: [],
    parts: [],
    touchFx: { selected: { fxType: "stutter", params: { rateHz: 8, depthPct: 50 }, targetKey: "master" }, assignments: [] },
    xyRelease: "sample-hold",
    ...overrides
  } as unknown as RuntimeConfig;
}
