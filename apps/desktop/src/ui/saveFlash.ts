export function saveFlashVisible(lastFlashAtMs: number | null, nowMs: number, durationMs = 650): boolean {
  return lastFlashAtMs !== null && nowMs - lastFlashAtMs < durationMs;
}
