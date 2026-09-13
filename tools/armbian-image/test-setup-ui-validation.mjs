import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { test } from 'node:test';

const ROOT = path.resolve(import.meta.dirname, '../..');
const UI_FILES = [
  'userpatches/overlay/usr/local/share/octessera-setup-ui/js/app.js',
  'tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui/js/app.js',
];

const loadValidator = (relativePath) => {
  const source = fs.readFileSync(path.join(ROOT, relativePath), 'utf8');
  const start = source.indexOf('const PASSWORD_MIN_LENGTH = 8;');
  const stateStart = source.indexOf('const state = {', start);
  const validatorStart = source.indexOf('const validateSsh = () =>', stateStart);
  const end = source.indexOf('const validationError = () =>', start);
  assert.notEqual(start, -1);
  assert.notEqual(stateStart, -1);
  assert.notEqual(validatorStart, -1);
  assert.notEqual(end, -1);
  const context = {
    __els: { accountPassword: {}, accountPasswordConfirm: {} },
    __state: { accountPassword: '', accountPasswordConfirm: '' },
  };
  new vm.Script(`const state = globalThis.__state; const els = globalThis.__els; ${source.slice(start, stateStart)} ${source.slice(validatorStart, end)}; globalThis.passwordPolicy = { whitespace: PASSWORD_WHITESPACE_RANGES, control: PASSWORD_CONTROL_RANGES }; globalThis.validateAccountPassword = validateAccountPassword;`).runInNewContext(context);
  return {
    policy: context.passwordPolicy,
    validate: (password, confirmation = password) => {
      context.__state.accountPassword = password;
      context.__state.accountPasswordConfirm = confirmation;
      return context.validateAccountPassword();
    },
  };
};

for (const relativePath of UI_FILES) {
  test(`browser password validation matches the backend policy: ${relativePath}`, () => {
    const { policy, validate } = loadValidator(relativePath);
    assert.deepEqual(JSON.parse(JSON.stringify(policy.whitespace)), [[0x0009, 0x000d], [0x0020, 0x0020], [0x0085, 0x0085], [0x00a0, 0x00a0], [0x1680, 0x1680], [0x2000, 0x200a], [0x2028, 0x2029], [0x202f, 0x202f], [0x205f, 0x205f], [0x3000, 0x3000]]);
    assert.deepEqual(JSON.parse(JSON.stringify(policy.control)), [[0x0000, 0x001f], [0x007f, 0x009f], [0x00ad, 0x00ad], [0x061c, 0x061c], [0x180e, 0x180e], [0x200b, 0x200f], [0x202a, 0x202e], [0x2060, 0x206f], [0xfeff, 0xfeff]]);
    assert.equal(validate('pässw😀rd'), undefined);
    assert.equal(validate('😀'.repeat(8)), undefined);
    assert.equal(validate('😀'.repeat(128)), undefined);
    assert.notEqual(validate('\u0085'.repeat(8)), undefined);
    assert.notEqual(validate('\ufeff'.repeat(8)), undefined);
    assert.notEqual(validate('😀'.repeat(129)), undefined);
    assert.notEqual(validate('pässw😀rd', 'pässw😀rD'), undefined);
  });
}
