import type { RuntimeHostMessage, RuntimeRunnerMessage } from "@cellsymphony/device-contracts";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type PlaybackRuntimeConfig = {
  bpm: number;
  syncSource: "internal" | "external";
  midiClockOutEnabled: boolean;
  midiOutEnabled: boolean;
};

export type RuntimeMessagesBatch = {
  seq: number;
  messages: RuntimeRunnerMessage[];
};

const IPC_TIMEOUT = 4_000;

function withTimeout<R>(promise: Promise<R>, ms: number): Promise<R> {
  return Promise.race([
    promise,
    new Promise<R>((_, reject) => setTimeout(() => reject(new Error(`Tauri IPC timed out after ${ms}ms`)), ms))
  ]);
}

export class TauriCoreRunnerClient {
  async dispatch(message: RuntimeHostMessage): Promise<RuntimeRunnerMessage[]> {
    return (await withTimeout(invoke("core_runner_dispatch", { message }), IPC_TIMEOUT)) as RuntimeRunnerMessage[];
  }

  async reset(): Promise<void> {
    await withTimeout(invoke("core_runner_reset"), IPC_TIMEOUT);
  }

  async dispatchRuntime(message: RuntimeHostMessage): Promise<RuntimeRunnerMessage[]> {
    return (await withTimeout(invoke("runtime_dispatch", { message }), IPC_TIMEOUT)) as RuntimeRunnerMessage[];
  }

  async syncConfig(config: PlaybackRuntimeConfig): Promise<void> {
    await withTimeout(invoke("runtime_sync_config", { config }), IPC_TIMEOUT);
  }

  async handleMidiRealtime(bytes: Uint8Array): Promise<RuntimeRunnerMessage[]> {
    return (await withTimeout(invoke("runtime_handle_midi_realtime", { bytes: Array.from(bytes) }), IPC_TIMEOUT)) as RuntimeRunnerMessage[];
  }

  async advance(elapsedMs: number): Promise<RuntimeRunnerMessage[]> {
    await withTimeout(invoke("runtime_advance", { elapsedMs }), IPC_TIMEOUT);
    return [];
  }

  async drainRuntimeMessages(): Promise<RuntimeMessagesBatch[]> {
    return (await withTimeout(invoke("runtime_drain_messages"), IPC_TIMEOUT)) as RuntimeMessagesBatch[];
  }

  async listenRuntimeMessages(handler: (batch: RuntimeMessagesBatch) => void): Promise<() => void> {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return () => {};
    const unlisten = await listen<RuntimeMessagesBatch>("runtime_messages", (evt) => {
      handler({
        seq: Number(evt.payload?.seq ?? 0),
        messages: Array.isArray(evt.payload?.messages) ? evt.payload.messages : []
      });
    });
    return unlisten;
  }
}

export const tauriCoreRunner = new TauriCoreRunnerClient();
