import test from "node:test";
import assert from "node:assert/strict";

import { loadDefaultMappingConfig, mapIntentsToMusicalEvents, type MappingConfig } from "../src/index";

test("mapIntentsToMusicalEvents maps scanned intents to scanned target", () => {
  const config = loadDefaultMappingConfig();
  const { events } = mapIntentsToMusicalEvents([{ x: 2, y: 3, degree: 5, kind: "scanned" }], config);

  assert.equal(events.length, 1);
  assert.equal(events[0].type, "note_on");
  if (events[0].type === "note_on") {
    assert.equal(events[0].channel, config.scanned.channel);
    assert.ok(events[0].note >= config.baseMidiNote);
  }
});

test("mapIntentsToMusicalEvents supports note_off and none actions", () => {
  const config = loadDefaultMappingConfig();
  const offEvents = mapIntentsToMusicalEvents([{ x: 0, y: 0, degree: 0, kind: "deactivate" }], config);
  assert.equal(offEvents.events.length, 1);
  assert.equal(offEvents.events[0].type, "note_off");
  const noneResult = mapIntentsToMusicalEvents([{ x: 0, y: 0, degree: 0, kind: "stable" }], config);
  assert.equal(noneResult.events.length, 0);
  assert.equal(noneResult.intents.length, 0);
});

test("mapping config sanitizes out-of-range values", () => {
  const unsafe: MappingConfig = {
    baseMidiNote: -5,
    maxMidiNote: 999,
    rangeMode: "wrap",
    scale: [0, 2, 4, 7, 12],
    rowStepDegrees: -2,
    columnStepDegrees: 3.9,
    activate: { action: "note_on", channel: 20, velocity: 200, durationMs: 999999 },
    deactivate: { action: "note_off", channel: -1, velocity: 0, durationMs: 0 },
    stable: { action: "none", channel: 2, velocity: 80, durationMs: 120 },
    scanned: { action: "note_on", channel: 3, velocity: 80, durationMs: 120 },
    scanned_empty: { action: "note_off", channel: 99, velocity: 80, durationMs: 120 }
  };

  const { events } = mapIntentsToMusicalEvents([{ x: 0, y: 0, degree: 300, kind: "activate" }], unsafe);
  assert.equal(events.length, 1);
  if (events[0].type === "note_on") {
    assert.ok(events[0].note >= 0 && events[0].note <= 127);
    assert.equal(events[0].channel, 15);
    assert.equal(events[0].velocity, 127);
    assert.equal(events[0].durationMs, 8000);
  }
});

test("range mode clamp and wrap behave differently for high degree", () => {
  const base = loadDefaultMappingConfig();
  const clampCfg: MappingConfig = { ...base, rangeMode: "clamp", maxMidiNote: base.baseMidiNote + 12 };
  const wrapCfg: MappingConfig = { ...base, rangeMode: "wrap", maxMidiNote: base.baseMidiNote + 12 };
  const degree = 400;
  const { events: clampEvents } = mapIntentsToMusicalEvents([{ x: 0, y: 0, degree, kind: "activate" }], clampCfg);
  const { events: wrapEvents } = mapIntentsToMusicalEvents([{ x: 0, y: 0, degree, kind: "activate" }], wrapCfg);
  assert.equal(clampEvents.length, 1);
  assert.equal(wrapEvents.length, 1);
  if (clampEvents[0].type === "note_on" && wrapEvents[0].type === "note_on") {
    assert.notEqual(clampEvents[0].note, wrapEvents[0].note);
    assert.equal(clampEvents[0].note, clampCfg.maxMidiNote);
  }
});

test("empty scale throws validation error", () => {
  const bad = { ...loadDefaultMappingConfig(), scale: [] } as MappingConfig;
  assert.throws(() => mapIntentsToMusicalEvents([{ x: 0, y: 0, degree: 0, kind: "activate" }], bad).events);
});
