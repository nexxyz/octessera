import { PAN_POSITION_COUNT } from "@cellsymphony/device-contracts";
import { invoke } from "@tauri-apps/api/core";

export interface NativeAudioBridge {
  setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number; masterVolume: number }): Promise<void>;
  setRuntimePolicy(policy: { voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive" }): Promise<void>;
}

class TauriNativeAudioBridge implements NativeAudioBridge {
  async setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number; masterVolume: number }): Promise<void> {
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!isTauri) return;
    const payload = config ?? { instruments: [], mixer: { buses: [] }, panPositions: PAN_POSITION_COUNT, masterVolume: 100 };
    await invoke("audio_set_instruments", { config: payload });
  }

  async setRuntimePolicy(policy: { voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive" }): Promise<void> {
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!isTauri) return;
    await invoke("audio_set_runtime_policy", { policy });
  }
}

export const nativeAudioBridge: NativeAudioBridge = new TauriNativeAudioBridge();
