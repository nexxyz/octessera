import assert from 'node:assert/strict';
import test from 'node:test';
import { normalizeRuntimePayload } from '../src/runtime/runner/tauriCoreRunner';

test('normalizes valid dispatch messages without changing their order or contents', () => {
  const messages = [
    { type: 'opaque', value: 1 },
    { type: 'opaque', value: 1 },
    null,
  ];

  const normalized = normalizeRuntimePayload(messages, 'messages');

  assert.strictEqual(normalized, messages);
  assert.deepEqual(normalized, messages);
});

test('normalizes malformed dispatch payloads to an empty message list', () => {
  assert.deepEqual(normalizeRuntimePayload(null, 'messages'), []);
  assert.deepEqual(normalizeRuntimePayload({ messages: [] }, 'messages'), []);
});

test('normalizes valid drained batches and preserves numeric sequence values', () => {
  const messages = [{ type: 'opaque', value: 1 }];
  const normalized = normalizeRuntimePayload(
    [
      { seq: '12', messages },
      { seq: 13, messages },
    ],
    'batches',
  );

  assert.deepEqual(normalized, [
    { seq: 12, messages },
    { seq: 13, messages },
  ]);
  assert.strictEqual(normalized[0]?.messages, messages);
  assert.strictEqual(normalized[1]?.messages, messages);
});

test('normalizes malformed drained payloads and uses zero for missing sequences', () => {
  assert.deepEqual(normalizeRuntimePayload(null, 'batches'), []);
  assert.deepEqual(
    normalizeRuntimePayload(
      [{ messages: 'not-a-list' }, null, { seq: null, messages: [] }],
      'batches',
    ),
    [
      { seq: 0, messages: [] },
      { seq: 0, messages: [] },
      { seq: 0, messages: [] },
    ],
  );
});

test('normalizes valid and malformed listen payloads through the same batch boundary', () => {
  const messages = [{ type: 'opaque', value: 2 }];

  assert.deepEqual(normalizeRuntimePayload({ seq: '21', messages }, 'batch'), {
    seq: 21,
    messages,
  });
  assert.deepEqual(normalizeRuntimePayload(undefined, 'batch'), {
    seq: 0,
    messages: [],
  });
});

test('normalizes invalid batch sequences to zero', () => {
  const invalidSequences: unknown[] = [
    'not-a-number',
    -1,
    1.5,
    Infinity,
    NaN,
    Number.MAX_SAFE_INTEGER + 1,
  ];

  for (const seq of invalidSequences) {
    assert.equal(
      normalizeRuntimePayload({ seq, messages: [] }, 'batch').seq,
      0,
    );
  }
});

test('preserves valid nonnegative safe integer batch sequences', () => {
  assert.equal(
    normalizeRuntimePayload(
      { seq: Number.MAX_SAFE_INTEGER, messages: [] },
      'batch',
    ).seq,
    Number.MAX_SAFE_INTEGER,
  );
  assert.equal(
    normalizeRuntimePayload({ seq: '42', messages: [] }, 'batch').seq,
    42,
  );
});
