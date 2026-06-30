import type { RuntimeSnapshot } from "@cellsymphony/device-contracts";
import { tauriCoreRunner } from "./runner/tauriCoreRunner";

export type PlaybackConfigSyncState = {
  lastSyncedPlaybackConfig: string;
};

export function syncPlaybackConfigIfNeeded(frame: RuntimeSnapshot, state: PlaybackConfigSyncState, hasInjectedDispatch: boolean): void {
  if (hasInjectedDispatch) return;
  const midi = frame.settings?.midi;
  if (!midi) return;
  const config = {
    bpm: Number(frame.transport.bpm ?? 120),
    syncSource: midi.syncMode === "external" ? "external" : "internal",
    midiClockOutEnabled: Boolean(midi.clockOutEnabled),
    midiOutEnabled: Boolean(midi.enabled && midi.outId),
  } as const;
  const signature = JSON.stringify(config);
  if (signature === state.lastSyncedPlaybackConfig) return;
  state.lastSyncedPlaybackConfig = signature;
  void tauriCoreRunner.syncConfig(config).catch((err) => {
    console.error("[Runtime] syncConfig failed:", err);
    if (state.lastSyncedPlaybackConfig === signature) state.lastSyncedPlaybackConfig = "";
  });
}
