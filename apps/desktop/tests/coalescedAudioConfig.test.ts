import test from "node:test";
import assert from "node:assert/strict";
import { createCoalescedAudioConfigSender } from "../src/audio/coalescedAudioConfig";

const baseConfig = (value: number) => ({ instruments: [{ type: "synth", value }], mixer: { buses: [] }, panPositions: 33, masterVolume: 100 });

test("coalesced audio config sender sends only latest pending config", async () => {
  const sent: unknown[] = [];
  const sender = createCoalescedAudioConfigSender((config) => {
    sent.push(config);
  }, 10);

  assert.equal(sender.schedule(baseConfig(1)), true);
  assert.equal(sender.schedule(baseConfig(2)), true);
  assert.equal(sender.schedule(baseConfig(3)), true);
  assert.equal(sent.length, 0);

  await new Promise((resolve) => setTimeout(resolve, 30));
  assert.equal(sent.length, 1);
  assert.deepEqual(sent[0], baseConfig(3));
});

test("coalesced audio config sender ignores unchanged signatures", () => {
  const sent: unknown[] = [];
  const sender = createCoalescedAudioConfigSender((config) => {
    sent.push(config);
  }, 1000);

  assert.equal(sender.schedule(baseConfig(1)), true);
  assert.equal(sender.schedule(baseConfig(1)), false);
  sender.flush();
  assert.equal(sent.length, 1);
});
