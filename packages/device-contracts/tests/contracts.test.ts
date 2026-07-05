import test from "node:test";
import assert from "node:assert/strict";

import {
  GRID_DOMAIN,
  AUX_ENCODER_COUNT,
  cutoffDisplayToHz,
  GRID_HEIGHT,
  GRID_WIDTH,
  MIDI_REALTIME_MESSAGE_TYPES,
  OLED_HEIGHT,
  OLED_WIDTH,
  PAN_POSITION_COUNT,
  PLATFORM_CAPS,
  RUNTIME_STATUS_STATES,
  RUNTIME_TRANSPORT_STATES,
  SHARED_RUNTIME_CONTRACT_FIXTURES,
  type OledFrame
} from "../src/index";

test("grid constants are 8x8", () => {
  assert.equal(GRID_WIDTH, 8);
  assert.equal(GRID_HEIGHT, 8);
});

test("platform capabilities match the hardware profile", () => {
  assert.deepEqual(PLATFORM_CAPS, {
    gridWidth: 8,
    gridHeight: 8,
    partCount: 8,
    instrumentCount: 8,
    sampleSlotCount: 8,
    audioSampleRate: 44100,
    audioBlockFrames: 128,
    maxSynthVoices: 16,
    maxSampleVoices: 64,
    maxSynthVoicesPerSlot: 8,
    maxSampleVoicesPerSlot: 8,
    busFxWarningSlotCount: 6,
    busCount: 4,
    globalFxSlotCount: 2,
    auxEncoderCount: 3,
    touchFxMaxConcurrent: 2,
    scanSectionCounts: [1, 2, 4, 8],
    panPositionCount: 33,
    oledWidth: 128,
    oledHeight: 128
  });
  assert.equal(AUX_ENCODER_COUNT, PLATFORM_CAPS.auxEncoderCount);
  assert.equal(PAN_POSITION_COUNT, PLATFORM_CAPS.panPositionCount);
  assert.equal(OLED_WIDTH, PLATFORM_CAPS.oledWidth);
  assert.equal(OLED_HEIGHT, PLATFORM_CAPS.oledHeight);
});

test("OLED framebuffer uses 128x128 rgb565be", () => {
  const frame: OledFrame = { width: OLED_WIDTH, height: OLED_HEIGHT, format: "rgb565be", pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2) };
  assert.equal(frame.pixels.length, 32768);
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

test("runtime contract fixtures cover each host and runner message class", () => {
  assert.equal(MIDI_REALTIME_MESSAGE_TYPES.join(","), "clock,start,continue,stop");
  assert.equal(RUNTIME_STATUS_STATES.join(","), "idle,running,paused,error");
  assert.equal(RUNTIME_TRANSPORT_STATES.join(","), "stopped,playing,paused");

  const hostTypes = new Set<string>();
  const runnerTypes = new Set<string>();

  for (const fixture of SHARED_RUNTIME_CONTRACT_FIXTURES) {
    assert.ok(fixture.id.length > 0);
    assert.ok(fixture.description.length > 0);
    assert.ok(fixture.hostMessages.length > 0);
    assert.ok(fixture.runnerMessages.length > 0);

    for (const message of fixture.hostMessages) {
      hostTypes.add(message.type);
      if (message.type === "transport_pulse_step") assert.ok(message.pulses > 0);
      if (message.type === "midi_realtime_clock") assert.ok(message.pulses > 0);
    }

    for (const message of fixture.runnerMessages) {
      runnerTypes.add(message.type);
      if (message.type === "runtime_status") assert.ok(message.status.currentPpqnPulse >= 0);
    }
  }

  assert.deepEqual([...hostTypes].sort(), ["device_input", "midi_realtime_clock", "midi_realtime_continue", "midi_realtime_start", "midi_realtime_stop", "runtime_result", "transport_pulse_step"]);
  assert.deepEqual([...runnerTypes].sort(), ["audio_commands", "musical_events", "platform_effects", "runtime_status", "snapshot", "ui_pulse"]);
});

test("cutoff display clamps and scales into synth Hz range", () => {
  assert.equal(cutoffDisplayToHz(-50), 80);
  assert.equal(cutoffDisplayToHz(0), 80);
  assert.equal(cutoffDisplayToHz(255), 16000);
  const midpoint = cutoffDisplayToHz(128);
  assert.ok(midpoint > 80);
  assert.ok(midpoint < 16000);
});
