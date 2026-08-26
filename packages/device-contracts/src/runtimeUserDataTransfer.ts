export const RUNTIME_USER_DATA_TRANSFER_PHASES = [
  "ready",
  "closed",
  "unsupported",
] as const;
export type RuntimeUserDataTransferPhase =
  (typeof RUNTIME_USER_DATA_TRANSFER_PHASES)[number];

export const USER_DATA_TRANSFER_CODE_LENGTH = 10;
export const USER_DATA_TRANSFER_CODE_PATTERN =
  /^[23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz]{10}$/;

type RuntimeUserDataTransferStatusBase = {
  type: "user_data_transfer_status";
};

export type RuntimeUserDataTransferStatus =
  | (RuntimeUserDataTransferStatusBase & {
      phase: "ready";
      url: string;
      code: string;
      expiresInSeconds: number;
    })
  | (RuntimeUserDataTransferStatusBase & { phase: "closed" })
  | (RuntimeUserDataTransferStatusBase & { phase: "unsupported" });

const RUNTIME_USER_DATA_TRANSFER_STATUS_KEYS = new Set([
  "type",
  "phase",
  "url",
  "code",
  "expiresInSeconds",
]);

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasOwn(value: Record<string, unknown>, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function isHttpUrl(value: unknown): value is string {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > 128 ||
    !value.startsWith("http://") ||
    /[\s\u0000-\u001f\u007f]/.test(value)
  )
    return false;
  const authority = value.slice(7).split(/[/?#]/, 1)[0] ?? "";
  if (authority.length === 0 || authority.includes("@")) return false;
  if (authority.startsWith("[")) {
    const end = authority.indexOf("]");
    if (end <= 1) return false;
    const port = authority.slice(end + 1);
    return port === "" || (port.startsWith(":") && isValidPort(port.slice(1)));
  }
  const separator = authority.lastIndexOf(":");
  if (separator < 0) return true;
  if (separator === 0 || authority.slice(0, separator).includes(":")) return false;
  return isValidPort(authority.slice(separator + 1));
}

function isValidPort(value: string): boolean {
  const port = Number(value);
  return /^[0-9]+$/.test(value) && Number.isInteger(port) && port >= 1 && port <= 65535;
}

function isReadyStatus(
  value: Record<string, unknown>,
): value is RuntimeUserDataTransferStatus & { phase: "ready" } {
  return (
    isHttpUrl(value.url) &&
    typeof value.code === "string" &&
    USER_DATA_TRANSFER_CODE_PATTERN.test(value.code) &&
    typeof value.expiresInSeconds === "number" &&
    Number.isInteger(value.expiresInSeconds) &&
    value.expiresInSeconds >= 1 &&
    value.expiresInSeconds <= 900
  );
}

export function isRuntimeUserDataTransferStatus(
  value: unknown,
): value is RuntimeUserDataTransferStatus {
  if (
    !isRecord(value) ||
    value.type !== "user_data_transfer_status" ||
    !Object.keys(value).every((key) =>
      RUNTIME_USER_DATA_TRANSFER_STATUS_KEYS.has(key),
    ) ||
    !RUNTIME_USER_DATA_TRANSFER_PHASES.includes(
      value.phase as RuntimeUserDataTransferPhase,
    )
  )
    return false;
  if (
    ["url", "code", "expiresInSeconds"].some(
      (key) => hasOwn(value, key) && value[key] === undefined,
    )
  )
    return false;
  if (value.phase === "ready") return isReadyStatus(value);
  return (
    value.url === undefined &&
    value.code === undefined &&
    value.expiresInSeconds === undefined
  );
}
