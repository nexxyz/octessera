import { PAN_POSITION_COUNT } from "@cellsymphony/device-contracts";
import { invoke } from "@tauri-apps/api/core";

export interface NativeAudioBridge {
  setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number; masterVolume: number }): Promise<void>;
  setRuntimePolicy(policy: { voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none" }): Promise<void>;
}

class TauriNativeAudioBridge implements NativeAudioBridge {
  async setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number; masterVolume: number }): Promise<void> {
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!isTauri) return;
    const payload = config ?? { instruments: [], mixer: { buses: [] }, panPositions: PAN_POSITION_COUNT, masterVolume: 100 };
    await invoke("audio_set_instruments", { config: payload });
  }

  async setRuntimePolicy(policy: { voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none" }): Promise<void> {
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!isTauri) return;
    await invoke("audio_set_runtime_policy", { policy });
  }
}

export const nativeAudioBridge: NativeAudioBridge = new TauriNativeAudioBridge();
