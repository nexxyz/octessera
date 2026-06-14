import test from "node:test";
import assert from "node:assert/strict";

import {
  GRID_DOMAIN,
  GRID_HEIGHT,
  GRID_WIDTH,
  MIDI_REALTIME_MESSAGE_TYPES,
  RUNTIME_STATUS_STATES,
  RUNTIME_TRANSPORT_STATES,
  SHARED_RUNTIME_CONTRACT_FIXTURES,
  type OledFrame
} from "../src/index";

test("grid constants are 8x8", () => {
  assert.equal(GRID_WIDTH, 8);
  assert.equal(GRID_HEIGHT, 8);
});

test("OLED framebuffer uses 128x128 rgb565be", () => {
  const frame: OledFrame = { width: 128, height: 128, format: "rgb565be", pixels: new Uint8Array(128 * 128 * 2) };
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
      if (message.type === "midi_realtime" && message.message === "clock") assert.ok(message.pulses > 0);
    }

    for (const message of fixture.runnerMessages) {
      runnerTypes.add(message.type);
      if (message.type === "runtime_status") assert.ok(message.status.currentPpqnPulse >= 0);
    }
  }

  assert.deepEqual([...hostTypes].sort(), ["device_input", "midi_realtime", "runtime_result", "transport_pulse_step"]);
  assert.deepEqual([...runnerTypes].sort(), ["audio_commands", "musical_events", "platform_effects", "runtime_status", "snapshot"]);
});
