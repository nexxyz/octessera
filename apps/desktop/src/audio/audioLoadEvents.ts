import { listen } from '@tauri-apps/api/event';

export type AudioLoadStatus = {
  ratio: number;
  workerUtilization?: number;
  missedQuantumFlash: boolean;
};

export type AudioLoadPayload = {
  ratio?: number;
  workerUtilization?: number;
  missedQuantumFlash?: boolean;
};

export type AudioLoadService = {
  listenAudioLoad(
    handler: (status: AudioLoadStatus) => void,
  ): Promise<() => void>;
};

export class TauriAudioLoadService implements AudioLoadService {
  async listenAudioLoad(
    handler: (status: AudioLoadStatus) => void,
  ): Promise<() => void> {
    if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window))
      return () => {};
    const unlisten = await listen<AudioLoadPayload>('audio_load', (evt) => {
      handler(normalizeAudioLoadPayload(evt.payload));
    });
    return unlisten;
  }
}

export function normalizeAudioLoadPayload(
  payload: AudioLoadPayload | undefined,
): AudioLoadStatus {
  const ratio = Number(payload?.ratio ?? 0);
  return {
    ratio: Number.isFinite(ratio) ? ratio : 0,
    workerUtilization: normalizeWorkerUtilization(payload?.workerUtilization),
    missedQuantumFlash: payload?.missedQuantumFlash === true,
  };
}

function normalizeWorkerUtilization(
  value: number | undefined,
): number | undefined {
  return value !== undefined && Number.isFinite(value) && value >= 0
    ? value
    : undefined;
}
