import type { RuntimeSnapshot } from "@octessera/device-contracts";

const DIMMED_LED_FACTOR = 0.22;
const MIN_VISIBLE_NON_BLACK_CHANNEL = 8;
const MIN_DIMMED_VISIBLE_NON_BLACK_CHANNEL = 2;

export type GridLedColor = { r: number; g: number; b: number };

export function gridLedColor(frame: RuntimeSnapshot, index: number): GridLedColor {
  const offset = index * 3;
  const dimmed = frame.settings?.ledsDimmed ?? false;
  const factor = brightnessScale(frame.settings?.gridBrightness) * (dimmed ? DIMMED_LED_FACTOR : 1);
  return scaleVisibleColor([
    frame.leds.rgb[offset] ?? 0,
    frame.leds.rgb[offset + 1] ?? 0,
    frame.leds.rgb[offset + 2] ?? 0
  ], factor, dimmed ? MIN_DIMMED_VISIBLE_NON_BLACK_CHANNEL : MIN_VISIBLE_NON_BLACK_CHANNEL);
}

function brightnessScale(value: number | undefined): number {
  return value === undefined ? 1 : Math.min(100, Math.max(0, value)) / 100;
}

function scaleVisibleColor(rgb: [number, number, number], factor: number, minimumVisibleChannel: number): GridLedColor {
  if (rgb.every((channel) => channel <= 0)) return { r: 0, g: 0, b: 0 };
  const scaled = rgb.map((channel) => Math.round(Math.min(255, Math.max(0, channel)) * factor)) as [number, number, number];
  const brightest = Math.max(...scaled);
  if (brightest >= minimumVisibleChannel) return toColor(scaled);
  const visibleFactor = minimumVisibleChannel / Math.max(1, Math.max(...rgb));
  return toColor(rgb.map((channel) => Math.round(channel * visibleFactor)) as [number, number, number]);
}

function toColor(rgb: [number, number, number]): GridLedColor {
  return { r: rgb[0], g: rgb[1], b: rgb[2] };
}
