export const RUNTIME_SETUP_PORTAL_PHASES = [
  "starting",
  "portal_ready",
  "finalizing",
  "succeeded",
  "failed",
  "timed_out",
  "unsupported",
] as const;
export type RuntimeSetupPortalPhase =
  (typeof RUNTIME_SETUP_PORTAL_PHASES)[number];

export const RUNTIME_SETUP_PORTAL_DISPOSITIONS = [
  "accepted",
  "already_running",
] as const;
export type RuntimeSetupPortalDisposition =
  (typeof RUNTIME_SETUP_PORTAL_DISPOSITIONS)[number];

export const RUNTIME_SETUP_PORTAL_ERROR_CODES = [
  "operation_failed",
  "unavailable",
  "invalid_payload",
  "unsupported",
] as const;
export type RuntimeSetupPortalErrorCode =
  (typeof RUNTIME_SETUP_PORTAL_ERROR_CODES)[number];
export type RuntimeSetupPortalFailureErrorCode = Exclude<
  RuntimeSetupPortalErrorCode,
  "unsupported"
>;

export const SETUP_PORTAL_SUFFIX_MAX_CHARS = 4;

type RuntimeSetupPortalStatusTag = {
  type: "setup_portal_status";
  rebootRequired: false;
};

type RuntimeSetupPortalHexDigit =
  | "0"
  | "1"
  | "2"
  | "3"
  | "4"
  | "5"
  | "6"
  | "7"
  | "8"
  | "9"
  | "a"
  | "b"
  | "c"
  | "d"
  | "e"
  | "f";
export type RuntimeSetupPortalSuffix =
  `${RuntimeSetupPortalHexDigit}${RuntimeSetupPortalHexDigit}${RuntimeSetupPortalHexDigit}${RuntimeSetupPortalHexDigit}`;

export type RuntimeSetupPortalStatus =
  | (RuntimeSetupPortalStatusTag & {
      phase: "starting";
      disposition: RuntimeSetupPortalDisposition;
    })
  | (RuntimeSetupPortalStatusTag & {
      phase: "portal_ready";
      portalSuffix: RuntimeSetupPortalSuffix;
    })
  | (RuntimeSetupPortalStatusTag & { phase: "finalizing" })
  | (RuntimeSetupPortalStatusTag & { phase: "succeeded" })
  | (RuntimeSetupPortalStatusTag & {
      phase: "failed";
      errorCode: RuntimeSetupPortalFailureErrorCode;
    })
  | (RuntimeSetupPortalStatusTag & {
      phase: "timed_out";
      errorCode: "unavailable";
    })
  | (RuntimeSetupPortalStatusTag & {
      phase: "unsupported";
      errorCode: "unsupported";
    });

const RUNTIME_SETUP_PORTAL_STATUS_KEYS = new Set([
  "type",
  "phase",
  "disposition",
  "portalSuffix",
  "rebootRequired",
  "errorCode",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(value: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function isRuntimeSetupPortalPhase(
  value: unknown,
): value is RuntimeSetupPortalPhase {
  return (
    typeof value === "string" &&
    RUNTIME_SETUP_PORTAL_PHASES.includes(value as RuntimeSetupPortalPhase)
  );
}

function isRuntimeSetupPortalDisposition(
  value: unknown,
): value is RuntimeSetupPortalDisposition {
  return (
    typeof value === "string" &&
    RUNTIME_SETUP_PORTAL_DISPOSITIONS.includes(
      value as RuntimeSetupPortalDisposition,
    )
  );
}

function isRuntimeSetupPortalErrorCode(
  value: unknown,
): value is RuntimeSetupPortalErrorCode {
  return (
    typeof value === "string" &&
    RUNTIME_SETUP_PORTAL_ERROR_CODES.includes(
      value as RuntimeSetupPortalErrorCode,
    )
  );
}

export function isRuntimeSetupPortalSuffix(
  value: unknown,
): value is RuntimeSetupPortalSuffix {
  return typeof value === "string" && /^[0-9a-f]{4}$/.test(value);
}

export function isRuntimeSetupPortalStatus(
  value: unknown,
): value is RuntimeSetupPortalStatus {
  if (
    !isRecord(value) ||
    value.type !== "setup_portal_status" ||
    value.rebootRequired !== false
  )
    return false;
  if (
    !Object.keys(value).every((key) =>
      RUNTIME_SETUP_PORTAL_STATUS_KEYS.has(key),
    )
  )
    return false;
  if (
    ["disposition", "portalSuffix", "errorCode"].some(
      (key) => hasOwn(value, key) && value[key] === undefined,
    )
  )
    return false;
  if (!isRuntimeSetupPortalPhase(value.phase)) return false;
  if (!hasValidRuntimeSetupPortalOptionalFields(value)) return false;
  return hasValidRuntimeSetupPortalPhaseFields(value, value.phase);
}

function hasValidRuntimeSetupPortalOptionalFields(
  value: Record<string, unknown>,
): boolean {
  if (
    value.disposition !== undefined &&
    !isRuntimeSetupPortalDisposition(value.disposition)
  )
    return false;
  if (
    value.portalSuffix !== undefined &&
    !isRuntimeSetupPortalSuffix(value.portalSuffix)
  )
    return false;
  if (
    value.errorCode !== undefined &&
    !isRuntimeSetupPortalErrorCode(value.errorCode)
  )
    return false;
  return true;
}

function hasStartingSetupPortalFields(
  value: Record<string, unknown>,
): boolean {
  return (
    value.disposition !== undefined &&
    value.portalSuffix === undefined &&
    value.errorCode === undefined
  );
}

function hasPortalReadySetupPortalFields(
  value: Record<string, unknown>,
): boolean {
  return (
    value.disposition === undefined &&
    value.portalSuffix !== undefined &&
    value.errorCode === undefined
  );
}

function hasEmptySetupPortalFields(value: Record<string, unknown>): boolean {
  return (
    value.disposition === undefined &&
    value.portalSuffix === undefined &&
    value.errorCode === undefined
  );
}

function hasOnlyErrorCode(
  value: Record<string, unknown>,
  errorCode: RuntimeSetupPortalErrorCode,
): boolean {
  return (
    value.disposition === undefined &&
    value.portalSuffix === undefined &&
    value.errorCode === errorCode
  );
}

function hasValidRuntimeSetupPortalPhaseFields(
  value: Record<string, unknown>,
  phase: RuntimeSetupPortalPhase,
): boolean {
  switch (phase) {
    case "starting":
      return hasStartingSetupPortalFields(value);
    case "portal_ready":
      return hasPortalReadySetupPortalFields(value);
    case "finalizing":
    case "succeeded":
      return hasEmptySetupPortalFields(value);
    case "failed":
      return (
        hasOnlyErrorCode(value, "operation_failed") ||
        hasOnlyErrorCode(value, "unavailable") ||
        hasOnlyErrorCode(value, "invalid_payload")
      );
    case "timed_out":
      return hasOnlyErrorCode(value, "unavailable");
    case "unsupported":
      return hasOnlyErrorCode(value, "unsupported");
  }
}
