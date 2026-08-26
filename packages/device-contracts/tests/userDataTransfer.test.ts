import test from "node:test";
import assert from "node:assert/strict";

import {
  RUNTIME_USER_DATA_TRANSFER_PHASES,
  USER_DATA_TRANSFER_CODE_LENGTH,
  isRuntimeUserDataTransferStatus,
} from "../src/index";
import { RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES } from "./runtimeProtocolFixtures";

test("user-data transfer status fixtures cover the three native phases", () => {
  assert.deepEqual(RUNTIME_USER_DATA_TRANSFER_PHASES, [
    "ready",
    "closed",
    "unsupported",
  ]);
  assert.equal(USER_DATA_TRANSFER_CODE_LENGTH, 10);
  for (const status of RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES) {
    assert.equal(isRuntimeUserDataTransferStatus(status), true);
  }
});

test("ready status enforces the URL, transfer-code, and lifetime policy", () => {
  const ready = RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES[0];
  for (const status of [
    { ...ready, url: "https://192.168.42.1:8081" },
    { ...ready, url: "http://" },
    { ...ready, code: "0123456789" },
    { ...ready, code: "Ab2Cd3Ef4" },
    { ...ready, expiresInSeconds: 0 },
    { ...ready, expiresInSeconds: 901 },
    { ...ready, extra: true },
  ]) {
    assert.equal(isRuntimeUserDataTransferStatus(status), false);
  }
});

test("closed and unsupported statuses reject transfer fields", () => {
  const closed = RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES[1];
  const unsupported = RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES[2];
  assert.equal(
    isRuntimeUserDataTransferStatus({
      ...closed,
      url: "http://192.168.42.1:8081",
    }),
    false,
  );
  assert.equal(
    isRuntimeUserDataTransferStatus({
      ...unsupported,
      expiresInSeconds: undefined,
    }),
    false,
  );
});
