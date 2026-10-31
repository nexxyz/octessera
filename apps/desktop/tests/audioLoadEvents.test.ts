import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeAudioLoadPayload } from '../src/audio/audioLoadEvents';

test('audio load normalization preserves missed quantum true and clear flags', () => {
  assert.equal(
    normalizeAudioLoadPayload({
      ratio: 0.2,
      workerUtilization: 0.9,
      missedQuantumFlash: true,
    }).missedQuantumFlash,
    true,
  );
  assert.equal(
    normalizeAudioLoadPayload({
      ratio: 0.2,
      workerUtilization: 0.9,
      missedQuantumFlash: false,
    }).missedQuantumFlash,
    false,
  );
});
