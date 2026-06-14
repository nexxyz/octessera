import test from "node:test";
import assert from "node:assert/strict";
import { sendEventsToAudio } from "../src/runtime/outputAdapters/audioSink";

test("audio sink is a no-op compatibility shim", async () => {
  await assert.doesNotReject(async () => {
    await sendEventsToAudio([{ type: "note_on", channel: 0, note: 60, velocity: 120, durationMs: 100 } as any], 50);
  });
});

test("audio sink ignores master volume and event payloads safely", async () => {
  await assert.doesNotReject(async () => {
    await sendEventsToAudio([{ type: "note_on", channel: 0, note: 60, velocity: 1, durationMs: 100 } as any], -10);
  });
});
