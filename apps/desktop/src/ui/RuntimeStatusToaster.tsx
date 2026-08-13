import { useEffect, useMemo, useState } from 'react';
import type {
  RuntimeErrorMetadata,
  RuntimeStatus,
} from '@octessera/device-contracts';
import type { OledFrameCacheFault } from '../runtime/oledFrameCache';

const TOAST_TIMEOUT_MS = 7000;

export function runtimeErrorIdentity(error: RuntimeErrorMetadata): string {
  return [
    error.domain,
    error.code,
    error.operation,
    error.requestId ?? '',
    error.revision ?? '',
  ].join(':');
}

export function runtimeErrorCopy(error: RuntimeErrorMetadata): {
  title: string;
  message: string;
  recovery: string;
} {
  const title =
    error.domain === 'audio'
      ? 'Audio unavailable'
      : error.domain === 'midi'
        ? 'MIDI unavailable'
        : error.domain === 'sample'
          ? 'Sample unavailable'
          : error.domain === 'storage'
            ? 'Save unavailable'
            : 'Octessera needs a moment';
  const message =
    error.message ??
    (error.recovery === 'stop_and_silence'
      ? 'Playback stopped safely.'
      : 'The current setup is unchanged.');
  const recovery =
    error.recovery === 'retry'
      ? 'Try again when ready.'
      : error.recovery === 'retain_last_good'
        ? 'Your last working setup is still here.'
        : 'Playback is stopped safely. Try Play again when ready.';
  return { title, message, recovery };
}

export function RuntimeStatusToaster({
  status,
  oledFrameFault,
  oledFrameAvailable,
}: {
  status: RuntimeStatus | null;
  oledFrameFault: OledFrameCacheFault | null;
  oledFrameAvailable: boolean;
}) {
  const error = status?.state === 'error' ? status.error : undefined;
  const identity = error ? runtimeErrorIdentity(error) : null;
  const [visible, setVisible] = useState<RuntimeStatus | null>(null);
  const [queued, setQueued] = useState<RuntimeStatus | null>(null);
  const [dismissedIdentity, setDismissedIdentity] = useState<string | null>(
    null,
  );
  const [paused, setPaused] = useState(false);
  const copy = useMemo(
    () => (visible?.error ? runtimeErrorCopy(visible.error) : null),
    [visible],
  );

  useEffect(() => {
    if (!error || !identity) {
      setVisible(null);
      setQueued(null);
      setDismissedIdentity(null);
      return;
    }
    if (dismissedIdentity === identity) return;
    if (visible?.error && runtimeErrorIdentity(visible.error) === identity)
      return;
    if (visible?.error) {
      setQueued(status);
      return;
    }
    setVisible(status);
  }, [dismissedIdentity, error, identity, status, visible]);

  useEffect(() => {
    if (!visible || paused) return;
    const timeout = window.setTimeout(() => {
      const visibleIdentity = runtimeErrorIdentity(visible.error!);
      if (!queued) setDismissedIdentity(visibleIdentity);
      setVisible((current) => {
        if (
          !current?.error ||
          runtimeErrorIdentity(current.error) !== visibleIdentity
        )
          return current;
        return queued;
      });
      setQueued(null);
    }, TOAST_TIMEOUT_MS);
    return () => window.clearTimeout(timeout);
  }, [paused, queued, visible]);

  function dismiss() {
    const visibleIdentity = visible?.error
      ? runtimeErrorIdentity(visible.error)
      : identity;
    setDismissedIdentity(visibleIdentity);
    setVisible(null);
    setQueued(null);
  }

  function restore() {
    if (!status?.error) return;
    setDismissedIdentity(null);
    setVisible(status);
  }

  if (!error || !identity) {
    if (!oledFrameFault) return null;
    return (
      <aside className="runtime-status-region" aria-label="Runtime status">
        <div className="runtime-status-indicator" role="status">
          {oledFaultCopy(oledFrameFault, oledFrameAvailable)}
        </div>
      </aside>
    );
  }
  const showing = Boolean(visible?.error);

  return (
    <aside className="runtime-status-region" aria-label="Runtime status">
      {showing && copy ? (
        <div
          className="runtime-status-toast"
          role="alert"
          aria-live="assertive"
          onMouseEnter={() => setPaused(true)}
          onMouseLeave={() => setPaused(false)}
          onFocus={() => setPaused(true)}
          onBlur={() => setPaused(false)}
        >
          <div className="runtime-status-copy">
            <strong>{copy.title}</strong>
            <span>{copy.message}</span>
            <small>{copy.recovery}</small>
          </div>
          <button
            type="button"
            className="runtime-status-dismiss"
            onClick={dismiss}
            aria-label="Dismiss runtime error"
          >
            Dismiss
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="runtime-status-indicator"
          onClick={restore}
          aria-label="Show runtime error"
        >
          Runtime needs attention
        </button>
      )}
    </aside>
  );
}

export function oledFaultCopy(
  fault: OledFrameCacheFault,
  frameAvailable: boolean,
): string {
  if (!frameAvailable) return 'OLED frame unavailable; showing blank display.';
  return fault === 'missing'
    ? 'OLED frame missing; showing last good frame.'
    : fault === 'future'
      ? 'OLED frame is ahead of the snapshot; showing last good frame.'
      : fault === 'stale'
        ? 'OLED frame is stale; showing last good frame.'
        : fault === 'conflict'
          ? 'OLED frame conflict; showing last good frame.'
          : 'OLED frame invalid; showing last good frame.';
}
