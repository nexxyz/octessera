import { createGridDomain } from "./gridDomain";
import {
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
} from "./platformCapabilities.generated";
import type { RuntimeErrorMetadata } from "./runtimeErrors";

export type DeviceInput =
  | { type: "encoder_turn"; delta: number; id?: "main" | `aux${number}` }
  | { type: "encoder_press"; id?: "main" | `aux${number}` }
  | { type: "encoder_release"; id?: "main" | `aux${number}` }
  | { type: "button_a"; pressed?: boolean }
  | { type: "button_s"; pressed?: boolean }
  | { type: "button_shift"; pressed?: boolean }
  | { type: "button_fn"; pressed?: boolean }
  | { type: "button_combined_modifier"; pressed?: boolean }
  | { type: "midi_clock"; pulses: number }
  | { type: "midi_start" }
  | { type: "midi_continue" }
  | { type: "midi_stop" }
  | { type: "grid_press"; x: number; y: number }
  | { type: "grid_release"; x: number; y: number }
  | { type: "behavior_action"; actionType: string };

export type PageId = string;

export type OledDisplayLayout = "rows" | "card" | "list";

export type DisplayFrame = {
  page: PageId;
  bodyLayout: OledDisplayLayout;
  title: string;
  lines: string[];
  editing: boolean;
};

export type OledFrame = {
  width: typeof OLED_WIDTH;
  height: typeof OLED_HEIGHT;
  format: "rgb565be";
  pixels: Uint8Array;
};

export type OledFrameRevision = number & {
  readonly __oledFrameRevision: unique symbol;
};

export type RuntimeSnapshotFields = {
  display: DisplayFrame;
  leds: LedMatrixFrame;
  transport: TransportFrame;
  activeBehavior: string;
  gridInteraction: GridInteraction;
  neoKeyLeds: NeoKeyLeds;
  eventDotLevel: number;
  transportIcon: "play" | "pause" | "stop";
  transportFlash: "none" | "beat" | "measure";
  settings?: RuntimeSnapshotSettings;
  runtimeError?: RuntimeErrorMetadata;
};

export const GRID_DOMAIN = createGridDomain(GRID_WIDTH, GRID_HEIGHT);

export type LedMatrixFrame = {
  width: number;
  height: number;
  rgb: number[];
  active: boolean[];
};

export type NeoKeyLeds = {
  back: [number, number, number];
  space: [number, number, number];
  shift: [number, number, number];
  fn: [number, number, number];
};

export type GridInteraction = "paint" | "momentary";

export type TransportFrame = {
  playing: boolean;
  bpm: number;
  tick: number;
  ppqnPulse: number;
};

export type RuntimeSnapshotSettings = {
  displayBrightness: number;
  gridBrightness?: number;
  buttonBrightness: number;
  ledsDimmed?: boolean;
};

export type NativeRuntimeSnapshot = RuntimeSnapshotFields & {
  oledFrameRevision: OledFrameRevision;
};

export type RuntimeSnapshot = RuntimeSnapshotFields & {
  oled: OledFrame;
};

export type LocalBootstrapSnapshot = RuntimeSnapshotFields & {
  oled: OledFrame;
  oledFrameRevision: 0;
};
