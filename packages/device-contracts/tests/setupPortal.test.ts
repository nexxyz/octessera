import test from "node:test";
import assert from "node:assert/strict";

import {
  RUNTIME_SETUP_PORTAL_DISPOSITIONS,
  RUNTIME_SETUP_PORTAL_ERROR_CODES,
  RUNTIME_SETUP_PORTAL_PHASES,
  SETUP_PORTAL_SUFFIX_MAX_CHARS,
  isRuntimeSetupPortalStatus,
  isRuntimeSetupPortalSuffix,
  type RuntimeSetupPortalStatus,
} from "../src/index";
import {
  RUNTIME_SETUP_PORTAL_STATUS_FIXTURES,
  RUNTIME_STORE_RESULT_FIXTURES,
} from "./runtimeProtocolFixtures";

type AssertFalse<T extends false> = T;

type InvalidStartingSuffix = {
  type: "setup_portal_status";
  phase: "starting";
  disposition: "accepted";
  portalSuffix: "abcd";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidStartingWithoutDisposition = {
  type: "setup_portal_status";
  phase: "starting";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidPortalReadyWithoutSuffix = {
  type: "setup_portal_status";
  phase: "portal_ready";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidFinalizingError = {
  type: "setup_portal_status";
  phase: "finalizing";
  errorCode: "operation_failed";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidSucceededSuffix = {
  type: "setup_portal_status";
  phase: "succeeded";
  portalSuffix: "abcd";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidFailedUnsupported = {
  type: "setup_portal_status";
  phase: "failed";
  errorCode: "unsupported";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidTimedOutOperationFailed = {
  type: "setup_portal_status";
  phase: "timed_out";
  errorCode: "operation_failed";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidUnsupportedUnavailable = {
  type: "setup_portal_status";
  phase: "unsupported";
  errorCode: "unavailable";
  rebootRequired: false;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidTrueReboot = {
  type: "setup_portal_status";
  phase: "succeeded";
  rebootRequired: true;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
type InvalidNullOptional = {
  type: "setup_portal_status";
  phase: "succeeded";
  rebootRequired: false;
  errorCode: null;
} extends RuntimeSetupPortalStatus
  ? true
  : false;
const SETUP_PORTAL_STATUS_COMPILE_MATRIX_CHECK: AssertFalse<
  | InvalidStartingSuffix
  | InvalidStartingWithoutDisposition
  | InvalidPortalReadyWithoutSuffix
  | InvalidFinalizingError
  | InvalidSucceededSuffix
  | InvalidFailedUnsupported
  | InvalidTimedOutOperationFailed
  | InvalidUnsupportedUnavailable
  | InvalidTrueReboot
  | InvalidNullOptional
> = false;

test("setup portal status fixtures cover the typed lifecycle and identity boundary", () => {
  assert.equal(SETUP_PORTAL_STATUS_COMPILE_MATRIX_CHECK, false);
  assert.deepEqual(RUNTIME_SETUP_PORTAL_PHASES, [
    "starting",
    "portal_ready",
    "finalizing",
    "succeeded",
    "failed",
    "timed_out",
    "unsupported",
  ]);
  assert.deepEqual(RUNTIME_SETUP_PORTAL_DISPOSITIONS, [
    "accepted",
    "already_running",
  ]);
  assert.deepEqual(RUNTIME_SETUP_PORTAL_ERROR_CODES, [
    "operation_failed",
    "unavailable",
    "invalid_payload",
    "unsupported",
  ]);
  assert.equal(SETUP_PORTAL_SUFFIX_MAX_CHARS, 4);
  assert.deepEqual(
    RUNTIME_SETUP_PORTAL_STATUS_FIXTURES.map((status) => status.phase),
    [
      "starting",
      "starting",
      "portal_ready",
      "finalizing",
      "succeeded",
      "failed",
      "failed",
      "failed",
      "timed_out",
      "unsupported",
    ],
  );
  assert.deepEqual(
    JSON.parse(JSON.stringify(RUNTIME_SETUP_PORTAL_STATUS_FIXTURES)),
    [
      {
        type: "setup_portal_status",
        phase: "starting",
        disposition: "accepted",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "starting",
        disposition: "already_running",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "portal_ready",
        portalSuffix: "abcd",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "finalizing",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "succeeded",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "failed",
        errorCode: "operation_failed",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "failed",
        errorCode: "invalid_payload",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "failed",
        errorCode: "unavailable",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "timed_out",
        errorCode: "unavailable",
        rebootRequired: false,
      },
      {
        type: "setup_portal_status",
        phase: "unsupported",
        errorCode: "unsupported",
        rebootRequired: false,
      },
    ],
  );
  for (const status of RUNTIME_SETUP_PORTAL_STATUS_FIXTURES) {
    assert.equal(isRuntimeSetupPortalStatus(status), true);
    assert.equal(status.rebootRequired, false);
    const serialized = JSON.stringify(status);
    for (const secretField of [
      "password",
      "passphrase",
      "secret",
      "credential",
      "output",
    ]) {
      assert.equal(serialized.includes(secretField), false);
    }
  }
  const identified = RUNTIME_STORE_RESULT_FIXTURES.find(
    (result) =>
      result.type === "identified" && result.requestId === "setup-portal-1",
  );
  assert.ok(identified && identified.revision === 9);
  assert.equal(identified?.result.type, "setup_portal_status");
  assert.equal(isRuntimeSetupPortalSuffix("abcd"), true);
  for (const suffix of ["éééé", "ABCD", "abc", "abcde", "ab-g"]) {
    assert.equal(isRuntimeSetupPortalSuffix(suffix), false);
  }
  const malformed = [
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[0], portalSuffix: "abcd" },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[0], disposition: null },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[2], portalSuffix: "abc" },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[2], portalSuffix: null },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[2], portalSuffix: undefined },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[2], disposition: "accepted" },
    {
      ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[3],
      errorCode: "operation_failed",
    },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[4], portalSuffix: "abcd" },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[5], errorCode: "unsupported" },
    {
      ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[5],
      errorCode: "audio_thread_failed",
    },
    {
      ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[8],
      errorCode: "operation_failed",
    },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[9], errorCode: "unavailable" },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[4], rebootRequired: true },
    { ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[4], errorCode: null },
    {
      ...RUNTIME_SETUP_PORTAL_STATUS_FIXTURES[4],
      output: "secret-bearing helper output",
    },
  ];
  for (const status of malformed)
    assert.equal(isRuntimeSetupPortalStatus(status), false);
});
