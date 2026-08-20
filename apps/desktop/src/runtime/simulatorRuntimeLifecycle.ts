import type {
  AudioLoadService,
  AudioLoadStatus,
} from '../audio/audioLoadEvents';
import type { SimulatorSnapshot } from './types';

export type RuntimeLifecycle = {
  listenAudioLoad(service: AudioLoadService): void;
  start(): void;
  publish(): void;
  subscribe(listener: (snapshot: SimulatorSnapshot) => void): () => void;
  stop(stopTransport: (beforeSchedulerStop: () => void) => void): void;
};

type RuntimeLifecycleOptions = {
  getSnapshot: () => SimulatorSnapshot;
  onAudioLoad: (status: AudioLoadStatus) => void;
  cleanupPresentation: () => void;
};

export function createRuntimeLifecycle(
  options: RuntimeLifecycleOptions,
): RuntimeLifecycle {
  const listeners = new Set<(snapshot: SimulatorSnapshot) => void>();
  let audioLoadActive = true;
  let audioListenGeneration = 0;
  let audioListenPending = false;
  let audioUnlisten: (() => void) | null = null;
  let audioLoadService: AudioLoadService | null = null;

  function listenAudioLoad(service: AudioLoadService): void {
    if (audioListenPending || audioUnlisten !== null) return;
    audioLoadService = service;
    audioListenPending = true;
    const generation = ++audioListenGeneration;
    void service
      .listenAudioLoad((status) => {
        if (!audioLoadActive || generation !== audioListenGeneration) return;
        options.onAudioLoad(status);
      })
      .then((unlisten) => {
        audioListenPending = false;
        if (!audioLoadActive || generation !== audioListenGeneration) {
          unlisten();
          return;
        }
        audioUnlisten = unlisten;
      });
  }

  function stopAudioLoad(): void {
    audioLoadActive = false;
    audioListenGeneration += 1;
    audioListenPending = false;
    audioUnlisten?.();
    audioUnlisten = null;
  }

  function publish(): void {
    const snapshot = options.getSnapshot();
    for (const listener of listeners) listener(snapshot);
  }

  return {
    listenAudioLoad,
    start() {
      audioLoadActive = true;
      if (audioLoadService !== null) listenAudioLoad(audioLoadService);
    },
    publish,
    subscribe(listener) {
      listeners.add(listener);
      listener(options.getSnapshot());
      return () => listeners.delete(listener);
    },
    stop(stopTransport) {
      stopAudioLoad();
      stopTransport(() => {
        options.cleanupPresentation();
        publish();
      });
    },
  };
}
