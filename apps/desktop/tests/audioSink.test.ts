import test from "node:test";
import assert from "node:assert/strict";
import { sendEventsToAudio } from "../src/runtime/outputAdapters/audioSink";
import { nativeAudioBridge } from "../src/audio/nativeAudioBridge";

test("forwards note_on velocity without applying master volume", async () => {
  const calls: any[] = [];
  const original = nativeAudioBridge.trigger;
  nativeAudioBridge.trigger = async (event: any) => {
    calls.push(event);
  };
  try {
    await sendEventsToAudio([{ type: "note_on", channel: 0, note: 60, velocity: 120, durationMs: 100 } as any], 50);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].velocity, 120);
  } finally {
    nativeAudioBridge.trigger = original;
  }
});

test("forwards low note_on velocity unchanged", async () => {
  const calls: any[] = [];
  const original = nativeAudioBridge.trigger;
  nativeAudioBridge.trigger = async (event: any) => {
    calls.push(event);
  };
  try {
    await sendEventsToAudio([{ type: "note_on", channel: 0, note: 60, velocity: 1, durationMs: 100 } as any], -10);
    assert.equal(calls[0].velocity, 1);
  } finally {
    nativeAudioBridge.trigger = original;
  }
});
