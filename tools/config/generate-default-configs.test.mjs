import { deepStrictEqual, doesNotThrow, equal, throws } from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import {
  generateDefaultConfigs,
  PLATFORM_OVERRIDE_ALLOWLIST,
  validatePlatformOverride,
} from "./generate-default-configs.mjs";

const baseConfig = {
  kind: "octessera.config",
  revision: 0,
  mappingConfig: { scale: [0, 3, 5] },
  runtimeConfig: {
    activeBehavior: "life",
    buttonBrightness: 35,
    displayBrightness: 75,
    gridBrightness: 25,
    instruments: [{ type: "synth", sample: { slots: [{ path: null }] } }],
    layers: [{ name: "life", worlds: { behaviorId: "life" } }],
  },
};

test("allows declared device-local brightness overrides", () => {
  const root = makeFixture({
    desktop: {
      runtimeConfig: {
        buttonBrightness: 100,
        displayBrightness: 100,
        gridBrightness: 100,
      },
    },
  });
  try {
    doesNotThrow(() => generateDefaultConfigs(root));
    const generated = readJson(join(root, "config/generated/desktop/default.json"));
    equal(generated.runtimeConfig.buttonBrightness, 100);
    equal(generated.runtimeConfig.displayBrightness, 100);
    equal(generated.runtimeConfig.gridBrightness, 100);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("rejects musical platform override paths", () => {
  throws(
    () =>
      validatePlatformOverride("desktop", {
        runtimeConfig: { instruments: [{ type: "sampler" }] },
      }),
    /desktop platform override path runtimeConfig\.instruments is not allowed.*musical runtime.*sample.*instrument/,
  );
  throws(
    () => validatePlatformOverride("pi", { mappingConfig: { scale: [0, 2, 4] } }),
    /pi platform override path mappingConfig is not allowed.*mapping/,
  );
});

test("rejects malformed platform override paths and values", () => {
  throws(
    () =>
      validatePlatformOverride("desktop", {
        runtimeConfig: { displayBrightness: { value: 100 } },
      }),
    /desktop platform override value at runtimeConfig\.displayBrightness must be an integer from 0 through 100/,
  );
  throws(
    () =>
      validatePlatformOverride("pi", {
        runtimeConfig: { displayBrightness: "bright" },
      }),
    /pi platform override value at runtimeConfig\.displayBrightness must be an integer from 0 through 100/,
  );
});

test("generated platform configs preserve musical patch parity", () => {
  const root = makeFixture({
    desktop: { runtimeConfig: { buttonBrightness: 100, displayBrightness: 100, gridBrightness: 100 } },
    pi: { runtimeConfig: { buttonBrightness: 35, displayBrightness: 75, gridBrightness: 25 } },
  });
  try {
    equal(generateDefaultConfigs(root), false);
    equal(generateDefaultConfigs(root, true), false);

    const desktop = readJson(join(root, "config/generated/desktop/default.json"));
    const pi = readJson(join(root, "config/generated/pi/default.json"));
    const canonical = readJson(join(root, "config/default.json"));
    deepStrictEqual(stripDeviceLocalBrightness(desktop), stripDeviceLocalBrightness(pi));
    deepStrictEqual(canonical, pi);
    deepStrictEqual(desktop, deepMerge(baseConfig, readJson(join(root, "config/defaults/desktop.json"))));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function makeFixture(overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "octessera-config-"));
  mkdirSync(join(root, "config/defaults"), { recursive: true });
  mkdirSync(join(root, "config/generated/desktop"), { recursive: true });
  mkdirSync(join(root, "config/generated/pi"), { recursive: true });
  writeJson(join(root, "config/defaults/base.json"), baseConfig);
  writeJson(join(root, "config/defaults/desktop.json"), overrides.desktop ?? {});
  writeJson(join(root, "config/defaults/pi.json"), overrides.pi ?? {});
  return root;
}

function stripDeviceLocalBrightness(config) {
  const copy = structuredClone(config);
  for (const path of PLATFORM_OVERRIDE_ALLOWLIST) {
    const [, key] = path.split(".");
    delete copy.runtimeConfig[key];
  }
  return copy;
}

function deepMerge(base, override) {
  if (Array.isArray(base) || Array.isArray(override)) return structuredClone(override ?? base);
  if (isObject(base) && isObject(override)) {
    const merged = structuredClone(base);
    for (const [key, value] of Object.entries(override)) {
      merged[key] = deepMerge(base[key], value);
    }
    return merged;
  }
  return structuredClone(override ?? base);
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}
