import type { MusicalEvent } from "@cellsymphony/musical-events";
import { PAN_POSITION_COUNT } from "@cellsymphony/platform-core";
import { invoke } from "@tauri-apps/api/core";

export interface NativeAudioBridge {
  trigger(event: MusicalEvent): Promise<void>;
  setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number }): Promise<void>;
  setRuntimePolicy(policy: { voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive" }): Promise<void>;
}

class TauriNativeAudioBridge implements NativeAudioBridge {
  async trigger(event: MusicalEvent): Promise<void> {
    const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
    if (!isTauri) return;
    await invoke("trigger_musical_event", { event });
  }

  async setInstruments(config: { instruments: unknown[]; mixer: unknown; panPositions: number }): Promise<void> {
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
