import { listen } from '@tauri-apps/api/event';

export type AudioLoadStatus = { ratio: number; voiceSteal: boolean };

type AudioLoadPayload = { ratio?: number; voiceSteal?: boolean };

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
      const ratio = Number(evt.payload?.ratio ?? 0);
      handler({
        ratio: Number.isFinite(ratio) ? ratio : 0,
        voiceSteal: evt.payload?.voiceSteal === true,
      });
    });
    return unlisten;
  }
}
