import { useEffect, useMemo, useState } from "react";
import type { RuntimeErrorMetadata, RuntimeStatus } from "@octessera/device-contracts";

const TOAST_TIMEOUT_MS = 7000;

export function runtimeErrorIdentity(error: RuntimeErrorMetadata): string {
  return [error.domain, error.code, error.operation, error.requestId ?? "", error.revision ?? ""].join(":");
}

export function runtimeErrorCopy(error: RuntimeErrorMetadata): { title: string; message: string; recovery: string } {
  const title = error.domain === "audio" ? "Audio unavailable" : error.domain === "midi" ? "MIDI unavailable" : error.domain === "sample" ? "Sample unavailable" : error.domain === "storage" ? "Save unavailable" : "Octessera needs a moment";
  const message = error.message ?? (error.recovery === "stop_and_silence" ? "Playback stopped safely." : "The current setup is unchanged.");
  const recovery = error.recovery === "retry" ? "Try again when ready." : error.recovery === "retain_last_good" ? "Your last working setup is still here." : "Playback is stopped safely. Try Play again when ready.";
  return { title, message, recovery };
}

export function RuntimeStatusToaster({ status }: { status: RuntimeStatus | null }) {
  const error = status?.state === "error" ? status.error : undefined;
  const identity = error ? runtimeErrorIdentity(error) : null;
  const [visible, setVisible] = useState<RuntimeStatus | null>(null);
  const [queued, setQueued] = useState<RuntimeStatus | null>(null);
  const [dismissedIdentity, setDismissedIdentity] = useState<string | null>(null);
  const [paused, setPaused] = useState(false);
  const copy = useMemo(() => visible?.error ? runtimeErrorCopy(visible.error) : null, [visible]);

  useEffect(() => {
    if (!error || !identity) {
      setVisible(null);
      setQueued(null);
      setDismissedIdentity(null);
      return;
    }
    if (dismissedIdentity === identity) return;
    if (visible?.error && runtimeErrorIdentity(visible.error) === identity) return;
    if (visible?.error) {
      setQueued(status);
      return;
    }
    setVisible(status);
  }, [dismissedIdentity, error, identity, status, visible]);

  useEffect(() => {
    if (!visible || paused) return;
    const timeout = window.setTimeout(() => {
      setVisible((current) => {
        if (!current?.error || runtimeErrorIdentity(current.error) !== runtimeErrorIdentity(visible.error!)) return current;
        return queued;
      });
      setQueued(null);
    }, TOAST_TIMEOUT_MS);
    return () => window.clearTimeout(timeout);
  }, [paused, queued, visible]);

  function dismiss() {
    const visibleIdentity = visible?.error ? runtimeErrorIdentity(visible.error) : identity;
    setDismissedIdentity(visibleIdentity);
    setVisible(null);
    setQueued(null);
  }

  function restore() {
    if (!status?.error) return;
    setDismissedIdentity(null);
    setVisible(status);
  }

  if (!error || !identity) return null;
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
          <button type="button" className="runtime-status-dismiss" onClick={dismiss} aria-label="Dismiss runtime error">
            Dismiss
          </button>
        </div>
      ) : (
        <button type="button" className="runtime-status-indicator" onClick={restore} aria-label="Show runtime error">
          Runtime needs attention
        </button>
      )}
    </aside>
  );
}
