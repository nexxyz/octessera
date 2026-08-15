export type OledAsyncReferenceGrace = {
  begin(revision: number): void;
  cancel(): void;
  complete(revision: number): void;
  canComplete(revision: number): boolean;
  hasPending(): boolean;
};

const OLED_REFERENCE_GRACE_MS = 16;

export function createOledAsyncReferenceGrace(
  onExpire: () => void,
): OledAsyncReferenceGrace {
  let pendingRevision: number | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  function cancel() {
    pendingRevision = null;
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  return {
    begin(revision) {
      if (pendingRevision === revision) return;
      cancel();
      pendingRevision = revision;
      timer = setTimeout(() => {
        if (pendingRevision !== revision) return;
        pendingRevision = null;
        timer = null;
        onExpire();
      }, OLED_REFERENCE_GRACE_MS);
    },
    cancel,
    complete(revision) {
      if (pendingRevision === revision) cancel();
    },
    canComplete(revision) {
      return pendingRevision === revision;
    },
    hasPending() {
      return pendingRevision !== null;
    },
  };
}
