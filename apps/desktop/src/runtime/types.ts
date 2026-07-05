import type { RuntimeSnapshot } from "@cellsymphony/device-contracts";

export type InputAction =
  | { type: "device_input"; input: import("@cellsymphony/device-contracts").DeviceInput }
  | { type: "emergency_brake" }
  | { type: "shift"; active: boolean }
  | { type: "fn"; active: boolean };

export type SimulatorSnapshot = {
  frame: RuntimeSnapshot;
  neoKeyLeds: Record<"back" | "space" | "shift" | "fn", [number, number, number]>;
  masterVolume: number;
  voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none";
  audioLoad: { ratio: number; voiceSteal: boolean };
  audioError: string | null;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  audioConfigRevision?: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  displayBrightness: number;
  buttonBrightness: number;
};

export type RuntimeListener = (snapshot: SimulatorSnapshot) => void;
