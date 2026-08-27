import {
  isPositiveOledFrameRevision,
  type LocalBootstrapSnapshot,
  type NativeRuntimeSnapshot,
  type RuntimeHostMessage,
  type RuntimeOledFrameMessage,
  type RuntimeSnapshot,
  type RuntimeStatus,
} from '@octessera/device-contracts';
import type { AudioLoadStatus } from '../audio/audioLoadEvents';
import {
  acceptOledFrameReference,
  acceptedOledFrame,
  createOledFrameCache,
  ingestOledFrame,
  markOledFrameFault,
  type OledFrameCache,
  type OledFrameCacheFault,
} from './oledFrameCache';
import {
  createOledAsyncReferenceGrace,
  type OledAsyncReferenceGrace,
} from './oledAsyncReferenceGrace';
import {
  createInitialRuntimeSnapshot,
  createRuntimeSnapshotCache,
  mergeSnapshotSettings,
  snapshotFromCore,
  type RuntimeSnapshotCache,
} from './simulatorSnapshot';
import {
  scheduleStartupSplashRefresh,
  type StartupSplashTimer,
} from './simulatorStartupSplash';
import type { SimulatorSnapshot } from './types';

export type RuntimeReconciliation = {
  reconcileOledFrame(
    message: RuntimeOledFrameMessage,
    allowAsyncOledSplit: boolean,
  ): void;
  reconcileSnapshot(
    snapshot: NativeRuntimeSnapshot,
    allowAsyncOledSplit: boolean,
  ): void;
  reconcileRuntimeStatus(status: RuntimeStatus): void;
  finishMessageBatch(): void;
  getSnapshot(audioLoad: AudioLoadStatus): SimulatorSnapshot;
  stop(): void;
};

type RuntimeReconciliationOptions = {
  mirrorRuntimeMessage: (message: RuntimeHostMessage) => void;
  publishSnapshot: () => void;
};

export function createRuntimeReconciliation(
  options: RuntimeReconciliationOptions,
): RuntimeReconciliation {
  let latestFrame: RuntimeSnapshot | LocalBootstrapSnapshot =
    createInitialRuntimeSnapshot();
  let runtimeStatus: RuntimeStatus | null = null;
  let visibleOledFrameFault: OledFrameCacheFault | null = null;
  let startupSplashTimer: StartupSplashTimer = null;
  const snapshotCache: RuntimeSnapshotCache = createRuntimeSnapshotCache();
  const oledFrameCache = createOledFrameCache();
  const oledAsyncReferenceGrace = createOledAsyncReferenceGrace(() => {
    visibleOledFrameFault = oledFrameCache.fault;
    options.publishSnapshot();
  });

  function reconcileOledFrame(
    message: RuntimeOledFrameMessage,
    allowAsyncOledSplit: boolean,
  ): void {
    const completesPendingReference = canCompleteOledReference(
      oledAsyncReferenceGrace,
      oledFrameCache,
      message.revision,
      allowAsyncOledSplit,
    );
    ingestOledFrame(oledFrameCache, message);
    if (completesPendingReference) {
      completeOledAsyncReference(
        oledAsyncReferenceGrace,
        oledFrameCache,
        message.revision,
      );
    } else if (!allowAsyncOledSplit && oledAsyncReferenceGrace.hasPending()) {
      oledAsyncReferenceGrace.cancel();
    } else if (
      oledAsyncReferenceGrace.hasPending() &&
      (oledFrameCache.fault === 'malformed' ||
        oledFrameCache.fault === 'stale' ||
        oledFrameCache.fault === 'conflict')
    ) {
      oledAsyncReferenceGrace.cancel();
    }
  }

  function reconcileSnapshot(
    snapshot: NativeRuntimeSnapshot,
    allowAsyncOledSplit: boolean,
  ): void {
    const revision = snapshot.oledFrameRevision as unknown;
    if (isPositiveOledFrameRevision(revision)) {
      acceptOledFrameReference(oledFrameCache, revision);
      if (
        allowAsyncOledSplit &&
        oledFrameCache.fault === 'future' &&
        revision > oledFrameCache.acceptedRevision &&
        oledFrameCache.candidateRevision < revision
      ) {
        oledAsyncReferenceGrace.begin(revision);
      } else {
        oledAsyncReferenceGrace.cancel();
      }
    } else {
      oledAsyncReferenceGrace.cancel();
      markOledFrameFault(
        oledFrameCache,
        revision === undefined ? 'missing' : 'malformed',
      );
    }
    mergeSnapshotSettings(snapshot, latestFrame);
    latestFrame = {
      ...snapshot,
      oled: acceptedOledFrame(oledFrameCache),
    };
    startupSplashTimer = scheduleStartupSplashRefresh(
      latestFrame,
      startupSplashTimer,
      options.mirrorRuntimeMessage,
      () => {
        startupSplashTimer = null;
      },
    );
  }

  function finishMessageBatch(): void {
    if (!oledAsyncReferenceGrace.hasPending()) {
      visibleOledFrameFault = oledFrameCache.fault;
    }
  }

  return {
    reconcileOledFrame,
    reconcileSnapshot,
    reconcileRuntimeStatus(status) {
      runtimeStatus = status;
    },
    finishMessageBatch,
    getSnapshot(audioLoad) {
      return snapshotFromCore(
        latestFrame,
        snapshotCache,
        {
          audioLoad,
          runtimeStatus,
          oledFrameFault: visibleOledFrameFault,
          oledFrameAvailable: oledFrameCache.acceptedPixels !== null,
        },
        acceptedOledFrame(oledFrameCache),
      );
    },
    stop() {
      if (startupSplashTimer !== null) {
        clearTimeout(startupSplashTimer);
        startupSplashTimer = null;
      }
      oledAsyncReferenceGrace.cancel();
      visibleOledFrameFault = oledFrameCache.fault;
    },
  };
}

function canCompleteOledReference(
  grace: OledAsyncReferenceGrace,
  cache: OledFrameCache,
  revision: number,
  allowAsyncOledSplit: boolean,
): boolean {
  return (
    allowAsyncOledSplit &&
    grace.canComplete(revision) &&
    revision > cache.acceptedRevision &&
    cache.candidateRevision <= revision
  );
}

function completeOledAsyncReference(
  grace: OledAsyncReferenceGrace,
  cache: OledFrameCache,
  revision: number,
): void {
  if (cache.candidateRevision === revision && cache.candidatePixels !== null)
    acceptOledFrameReference(cache, revision);
  if (cache.acceptedRevision === revision && cache.fault === null) {
    grace.complete(revision);
  } else {
    grace.cancel();
  }
}
