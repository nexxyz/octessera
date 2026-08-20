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
  type OledFrameCacheFault,
} from './oledFrameCache';
import { createOledAsyncReferenceGrace } from './oledAsyncReferenceGrace';
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
    const completesPendingReference =
      allowAsyncOledSplit &&
      oledAsyncReferenceGrace.canComplete(message.revision) &&
      message.revision > oledFrameCache.acceptedRevision &&
      oledFrameCache.candidateRevision <= message.revision;
    ingestOledFrame(oledFrameCache, message);
    if (completesPendingReference) {
      if (
        oledFrameCache.candidateRevision === message.revision &&
        oledFrameCache.candidatePixels !== null
      ) {
        acceptOledFrameReference(oledFrameCache, message.revision);
      }
      if (
        oledFrameCache.acceptedRevision === message.revision &&
        oledFrameCache.fault === null
      ) {
        oledAsyncReferenceGrace.complete(message.revision);
      } else {
        oledAsyncReferenceGrace.cancel();
      }
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
