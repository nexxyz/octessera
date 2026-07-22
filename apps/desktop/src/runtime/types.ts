import type { RuntimeSnapshot, RuntimeStatus } from "@octessera/device-contracts";

export type InputAction =
  | { type: "device_input"; input: import("@octessera/device-contracts").DeviceInput }
  | { type: "emergency_brake" }
  | { type: "shift"; active: boolean }
  | { type: "fn"; active: boolean };

export type SimulatorSnapshot = {
  frame: RuntimeSnapshot;
  runtimeStatus: RuntimeStatus | null;
  neoKeyLeds: Record<"back" | "space" | "shift" | "fn", [number, number, number]>;
  masterVolume: number;
  voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none";
  audioLoad: { ratio: number; voiceSteal: boolean };
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
