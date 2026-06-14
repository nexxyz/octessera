import test from "node:test";
import assert from "node:assert/strict";

import { createCoreRunner } from "../src/index";

test("core runner returns snapshot and status for device input", () => {
  const runner = createCoreRunner();
  const messages = runner.dispatch({ type: "device_input", input: { type: "grid_press", x: 1, y: 2 } });

  assert.equal(messages.at(-2)?.type, "snapshot");
  assert.equal(messages.at(-1)?.type, "runtime_status");
});

test("core runner advances transport via explicit internal pulse steps", () => {
  const runner = createCoreRunner();
  runner.getState().transport.playing = true;
  const before = runner.getStatus().currentPpqnPulse;

  const messages = runner.dispatch({ type: "transport_pulse_step", pulses: 6, source: "internal" });
  const status = messages.at(-1);

  assert.ok(status && status.type === "runtime_status");
  assert.ok(status.status.currentPpqnPulse > before);
  assert.equal(status.status.transport, "playing");
});

test("core runner applies host runtime results back into platform-core", () => {
  const runner = createCoreRunner();
  const messages = runner.dispatch({ type: "runtime_result", result: { type: "list_presets_result", names: ["Factory"] } });

  assert.equal(messages.at(-2)?.type, "snapshot");
  assert.equal(messages.at(-1)?.type, "runtime_status");
  assert.deepEqual(runner.getState().system.presetNames, ["Factory"]);
});
