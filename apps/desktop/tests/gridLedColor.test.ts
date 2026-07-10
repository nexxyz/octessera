import test from "node:test";
import assert from "node:assert/strict";
import type { RuntimeSnapshot } from "@octessera/device-contracts";
import { gridLedColor } from "../src/ui/gridLedColor";

function frame(rgb: number[], settings: RuntimeSnapshot["settings"]): RuntimeSnapshot {
  return {
    oled: { width: 128, height: 128, format: "rgb565be", pixels: new Uint8Array(32768) },
    leds: { width: 8, height: 8, rgb: [...rgb, ...Array.from({ length: 64 * 3 - rgb.length }, () => 0)], active: Array.from({ length: 64 }, () => false) },
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
  assert.deepEqual(gridLedColor(frame([100, 50, 0], settings({ gridBrightness: 50 })), 0), { r: 50, g: 25, b: 0 });
});

test("desktop grid LED color keeps black black", () => {
  assert.deepEqual(gridLedColor(frame([0, 0, 0], settings({ gridBrightness: 0 })), 0), { r: 0, g: 0, b: 0 });
});

test("desktop grid LED color keeps non-black cells visible at zero brightness", () => {
  assert.deepEqual(gridLedColor(frame([0, 255, 0], settings({ gridBrightness: 0 })), 0), { r: 0, g: 8, b: 0 });
});

test("desktop grid LED color keeps dimmed cells visibly dimmer", () => {
  assert.deepEqual(gridLedColor(frame([0, 255, 0], settings({ gridBrightness: 0, ledsDimmed: true })), 0), { r: 0, g: 2, b: 0 });
});
