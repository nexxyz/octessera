import type { RuntimeRunnerMessage } from "@cellsymphony/device-contracts";
import type { TransientIndicatorState } from "./simulatorSnapshot";

export type IndicatorTimer = ReturnType<typeof setTimeout> | null;

export function createTransientIndicators(): TransientIndicatorState {
  return { eventDotUntilMs: 0, transportFlashUntilMs: 0, transportFlash: "none" };
}

export function resetTransientIndicators(indicators: TransientIndicatorState): void {
  indicators.eventDotUntilMs = 0;
  indicators.transportFlashUntilMs = 0;
  indicators.transportFlash = "none";
}

export function applyTransientIndicatorPulse(
  pulse: Extract<RuntimeRunnerMessage, { type: "ui_pulse" }>["pulse"],
  indicators: TransientIndicatorState,
  previousTimer: IndicatorTimer,
  publishSnapshot: () => void,
  expireSnapshot: () => void,
): IndicatorTimer {
  const now = performance.now();
  if (pulse.type === "trigger_pulse") {
    indicators.eventDotUntilMs = now + pulse.durationMs;
  } else if (pulse.type === "transport_flash") {
    indicators.transportFlash = pulse.flash;
    indicators.transportFlashUntilMs = now + pulse.durationMs;
  }
  if (previousTimer !== null) clearTimeout(previousTimer);
  publishSnapshot();
  const nextUntil = Math.max(indicators.eventDotUntilMs, indicators.transportFlashUntilMs);
  return setTimeout(expireSnapshot, Math.max(0, nextUntil - now) + 5);
}
