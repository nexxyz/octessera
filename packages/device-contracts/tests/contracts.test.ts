import test from "node:test";
import assert from "node:assert/strict";

import {
  GRID_DOMAIN,
  AUX_ENCODER_COUNT,
  DISPLAY_PALETTE,
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
  PAN_POSITION_COUNT,
  PLATFORM_CAPS,
  RUNTIME_ERROR_CODES,
  RUNTIME_ERROR_DOMAINS,
  RUNTIME_OPERATIONS,
  RUNTIME_RECOVERIES,
  type RuntimeErrorFacts,
  type RuntimeErrorMetadata,
  type RuntimeSnapshot,
  type OledFrame,
} from "../src/index";

const CANDIDATE_HEALTH_MARKER_FIXTURE = {
  schema_version: 1,
  pid: 4242,
  systemd_invocation_id: "inv-1",
  package_version: "0.7.0",
  board_profile: "raspberry-pi-zero-2w",
  ready_at_unix_ms: 1_700_000_000_123,
} as const;

test("display palette matches the canonical instrument colors", () => {
  assert.deepEqual(DISPLAY_PALETTE.green, {
    label: "Green",
    hex: "#63D23F",
    rgb: [99, 210, 63],
    rgb565: 0x6687,
  });
  assert.deepEqual(DISPLAY_PALETTE.red, {
    label: "Red",
    hex: "#DD82CD",
    rgb: [221, 130, 205],
    rgb565: 0xdc19,
  });
  assert.deepEqual(DISPLAY_PALETTE.blue, {
    label: "Blue",
    hex: "#35CFF2",
    rgb: [53, 207, 242],
    rgb565: 0x367e,
  });
  assert.deepEqual(DISPLAY_PALETTE.yellow, {
    label: "Yellow",
    hex: "#FFD447",
    rgb: [255, 212, 71],
    rgb565: 0xfea8,
  });
  assert.deepEqual(DISPLAY_PALETTE.gray, {
    label: "Gray",
    hex: "#C9CED6",
    rgb: [201, 206, 214],
    rgb565: 0xce7a,
  });
  assert.deepEqual(DISPLAY_PALETTE.white, {
    label: "White",
    hex: "#FFFFFF",
    rgb: [255, 255, 255],
    rgb565: 0xffff,
  });
  assert.deepEqual(DISPLAY_PALETTE.black, {
    label: "Black",
    hex: "#000000",
    rgb: [0, 0, 0],
    rgb565: 0x0000,
  });
});

test("grid constants are 8x8", () => {
  assert.equal(GRID_WIDTH, 8);
  assert.equal(GRID_HEIGHT, 8);
});

test("platform capabilities match the hardware profile", () => {
  assert.deepEqual(PLATFORM_CAPS, {
    gridWidth: 8,
    gridHeight: 8,
    layerCount: 8,
    instrumentCount: 8,
    sampleSlotCount: 8,
    audioSampleRate: 44100,
    audioBlockFrames: 256,
    synthSlotWorkers: 2,
    maxSynthVoices: 16,
    maxSampleVoices: 64,
    maxSynthVoicesPerSlot: 8,
    maxSampleVoicesPerSlot: 8,
    busFxWarningSlotCount: 12,
    busCount: 4,
    globalFxSlotCount: 2,
    auxEncoderCount: 3,
    sparksFxMaxConcurrent: 2,
    scanSectionCounts: [1, 2, 4, 8],
    panPositionCount: 33,
    oledWidth: 128,
    oledHeight: 128,
  });
  assert.equal(AUX_ENCODER_COUNT, PLATFORM_CAPS.auxEncoderCount);
  assert.equal(PAN_POSITION_COUNT, PLATFORM_CAPS.panPositionCount);
  assert.equal(OLED_WIDTH, PLATFORM_CAPS.oledWidth);
  assert.equal(OLED_HEIGHT, PLATFORM_CAPS.oledHeight);
});

test("OLED framebuffer uses device contract rgb565be dimensions", () => {
  const frame: OledFrame = {
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: "rgb565be",
    pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2),
  };
  assert.equal(frame.pixels.length, OLED_WIDTH * OLED_HEIGHT * 2);
});

test("grid domain clamps/floors and preserves immutability", () => {
  const a = GRID_DOMAIN.toLogicalCell({ x: -1.4, y: 999.2 });
  assert.equal(a.x, 0);
  assert.equal(a.y, 0);

  const cells = new Array(GRID_WIDTH * GRID_HEIGHT).fill(false);
  const set = GRID_DOMAIN.set(cells, { x: 2, y: 3 }, true);
  assert.equal(cells[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], false);
  assert.equal(set[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], true);

  const toggled = GRID_DOMAIN.toggle(set, { x: 2, y: 3 });
  assert.equal(toggled[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], false);
});

test("grid domain index conversion is consistent", () => {
  const idx = GRID_DOMAIN.toLogicalIndex({ x: 1, y: 2 });
  const cell = GRID_DOMAIN.cellOf(idx);
  assert.equal(cell.x, 1);
  assert.equal(cell.y, 5);
  const back = GRID_DOMAIN.toDisplayCell(cell);
  assert.equal(back.x, 1);
  assert.equal(back.y, 2);
});

test("grid display conversion preserves lower-left logical origin", () => {
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 0, y: 0 }), { x: 0, y: 7 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 7, y: 0 }), { x: 7, y: 7 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 0, y: 7 }), { x: 0, y: 0 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 7, y: 7 }), { x: 7, y: 0 });
});

test("candidate health marker fixture matches the guard identity contract", () => {
  assert.deepEqual(
    JSON.parse(JSON.stringify(CANDIDATE_HEALTH_MARKER_FIXTURE)),
    CANDIDATE_HEALTH_MARKER_FIXTURE,
  );
  assert.equal(CANDIDATE_HEALTH_MARKER_FIXTURE.schema_version, 1);
  assert.ok(CANDIDATE_HEALTH_MARKER_FIXTURE.pid > 0);
  assert.ok(CANDIDATE_HEALTH_MARKER_FIXTURE.systemd_invocation_id.length > 0);
  assert.equal(
    CANDIDATE_HEALTH_MARKER_FIXTURE.board_profile,
    "raspberry-pi-zero-2w",
  );
});

test("runtime error metadata serializes with stable typed identity and recovery", () => {
  const error = {
    domain: "storage",
    code: "operation_failed",
    operation: "store_load_default",
    recovery: "retain_last_good",
    message: "disk full",
  } as const satisfies RuntimeErrorMetadata;
  const snapshot = { runtimeError: error } as Pick<
    RuntimeSnapshot,
    "runtimeError"
  >;

  assert.deepEqual(JSON.parse(JSON.stringify(snapshot)), snapshot);
  const setupError = {
    domain: "runtime",
    code: "unsupported",
    operation: "setup_portal",
  } as const satisfies RuntimeErrorFacts;
  assert.deepEqual(JSON.parse(JSON.stringify(setupError)), setupError);
  assert.deepEqual(RUNTIME_ERROR_DOMAINS, [
    "runtime",
    "storage",
    "midi",
    "sample",
    "audio",
    "serialization",
  ]);
  assert.deepEqual(RUNTIME_ERROR_CODES, [
    "operation_failed",
    "unavailable",
    "invalid_payload",
    "not_found",
    "unsupported",
    "serialization_failed",
    "audio_thread_failed",
  ]);
  assert.ok(RUNTIME_OPERATIONS.includes(error.operation));
  assert.ok(RUNTIME_OPERATIONS.includes("device_update"));
  assert.ok(RUNTIME_OPERATIONS.includes("setup_portal"));
  assert.ok(RUNTIME_RECOVERIES.includes(error.recovery));
});
