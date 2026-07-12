import test from "node:test";
import assert from "node:assert/strict";
import { BLACK_COLOR, YELLOW_COLOR, GREEN_COLOR, type RuntimeSnapshot } from "@octessera/device-contracts";
import { gridLedColor } from "../src/ui/gridLedColor";

function frame(rgb: readonly number[], settings: RuntimeSnapshot["settings"], active = true): RuntimeSnapshot {
  return {
    oled: { width: 128, height: 128, format: "rgb565be", pixels: new Uint8Array(32768) },
    leds: { width: 8, height: 8, rgb: [...rgb, ...Array.from({ length: 64 * 3 - rgb.length }, () => 0)], active: Array.from({ length: 64 }, () => active) },
    transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
    display: { page: "boot", title: "Boot", lines: [], editing: false },
    activeBehavior: "life",
    gridInteraction: "paint",
    settings,
  };
}

const settings = (overrides: Partial<NonNullable<RuntimeSnapshot["settings"]>>): NonNullable<RuntimeSnapshot["settings"]> => ({
  displayBrightness: 75,
  gridBrightness: 100,
  buttonBrightness: 75,
  masterVolume: 73,
  voiceStealingMode: "auto-balanced",
  autoSaveFlash: "none",
  transportFlash: "none",
  stopLatched: false,
  shiftHeld: false,
  fnHeld: false,
  combinedModifierHeld: false,
  midi: { enabled: false, outId: null, inId: null, syncMode: "internal", clockOutEnabled: false, clockInEnabled: false },
  ...overrides,
});

test("desktop grid LED color applies grid brightness", () => {
  assert.deepEqual(gridLedColor(frame(YELLOW_COLOR, settings({ gridBrightness: 50 })), 0), { r: 128, g: 106, b: 36 });
});

test("desktop grid LED color keeps black black", () => {
  assert.deepEqual(gridLedColor(frame(BLACK_COLOR, settings({ gridBrightness: 0 })), 0), { r: 0, g: 0, b: 0 });
});

test("desktop grid LED color keeps inactive LEDs black", () => {
  assert.deepEqual(gridLedColor(frame(GREEN_COLOR, settings({ gridBrightness: 100 }), false), 0), { r: 0, g: 0, b: 0 });
});

test("desktop grid LED color keeps non-black cells visible at zero brightness", () => {
  assert.deepEqual(gridLedColor(frame(GREEN_COLOR, settings({ gridBrightness: 0 })), 0), { r: 4, g: 8, b: 2 });
});

test("desktop grid LED color keeps dimmed cells visibly dimmer", () => {
  assert.deepEqual(gridLedColor(frame(GREEN_COLOR, settings({ gridBrightness: 0, ledsDimmed: true })), 0), { r: 1, g: 2, b: 1 });
});
