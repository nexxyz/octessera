import { createGridDomain, type GridCell, type GridDomain } from "./gridDomain";
export { createGridDomain } from "./gridDomain";

export type DeviceInput =
  | { type: "encoder_turn"; delta: -1 | 1; id?: "main" | "aux1" | "aux2" | "aux3" | "aux4" }
  | { type: "encoder_press"; id?: "main" | "aux1" | "aux2" | "aux3" | "aux4" }
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

export type DisplayFrame = {
  page: PageId;
  title: string;
  lines: string[];
  editing: boolean;
  colors?: number[]; // RGB565 colors per line (optional, for OLED rendering)
};

export type OledFrame = {
  width: 128;
  height: 128;
  format: "rgb565be";
  pixels: Uint8Array;
};

export type LedCell = { r: number; g: number; b: number };
export const GRID_WIDTH = 8 as const;
export const GRID_HEIGHT = 8 as const;
export const GRID_DOMAIN = createGridDomain(GRID_WIDTH, GRID_HEIGHT);
export type { GridCell, GridDomain };
export type LedMatrixFrame = {
  width: number;
  height: number;
  cells: LedCell[];
};

export type GridInteraction = "paint" | "momentary";

export type TransportFrame = {
  playing: boolean;
  bpm: number;
  tick: number;
  ppqnPulse: number;
};

export type SimulatorFrame = {
  display: DisplayFrame;
  oled?: OledFrame;
  leds: LedMatrixFrame;
  transport: TransportFrame;
  activeBehavior: string;
  gridInteraction: GridInteraction;
};
