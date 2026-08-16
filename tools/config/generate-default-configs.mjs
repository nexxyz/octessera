import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");

export const PLATFORM_OVERRIDE_ALLOWLIST = Object.freeze([
  "runtimeConfig.buttonBrightness",
  "runtimeConfig.displayBrightness",
  "runtimeConfig.gridBrightness",
]);

const platformOverrideValueValidators = new Map(
  PLATFORM_OVERRIDE_ALLOWLIST.map((path) => [path, validateBrightness]),
);

const targets = [
  { name: "desktop", output: ["config", "generated", "desktop", "default.json"] },
  { name: "pi", output: ["config", "generated", "pi", "default.json"] },
];

export function generateDefaultConfigs(root, check = false) {
  const base = readJson(root, "config/defaults/base.json");
  let failed = false;

  for (const target of targets) {
    const overridePath = `config/defaults/${target.name}.json`;
    const override = readJson(root, overridePath);
    validatePlatformOverride(target.name, override);
    const generated = stableJson(deepMerge(base, override));
    const outputPath = resolve(root, ...target.output);
    if (check) {
      const existing = readFile(root, target.output.join("/"));
      if (existing !== generated) {
        console.error(`${target.output.join("/")} is out of date. Run corepack pnpm run config:generate.`);
        failed = true;
      }
    } else {
      writeFileSync(outputPath, generated);
    }
  }

  const piDefault = readFile(root, "config/generated/pi/default.json");
  const canonicalPath = resolve(root, "config", "default.json");
  if (check) {
    const existing = readFile(root, "config/default.json");
    if (existing !== piDefault) {
      console.error("config/default.json is out of date. Run corepack pnpm run config:generate.");
      failed = true;
    }
  } else {
    writeFileSync(canonicalPath, piDefault);
  }

  return failed;
}

export function validatePlatformOverride(targetName, override) {
  if (!isObject(override)) {
    throw new Error(
      `${targetName} platform override must be a JSON object; only device-local brightness paths are allowed.`,
    );
  }
  validateOverrideObject(targetName, override, "");
}

function validateOverrideObject(targetName, value, parentPath) {
  for (const [key, child] of Object.entries(value)) {
    const path = parentPath ? `${parentPath}.${key}` : key;
    const validator = platformOverrideValueValidators.get(path);
    if (validator) {
      validator(targetName, path, child);
      continue;
    }
    if (isObject(child)) {
      if (!isAllowedPrefix(path)) {
        throw invalidOverridePath(targetName, path);
      }
      validateOverrideObject(targetName, child, path);
      continue;
    }
    throw invalidOverridePath(targetName, path);
  }
}

function invalidOverridePath(targetName, path) {
  return new Error(
    `${targetName} platform override path ${path} is not allowed. Platform overrides are device-local only; allowed paths: ${PLATFORM_OVERRIDE_ALLOWLIST.join(", ")}. Keep musical runtime, mapping, sample, instrument, layer, FX, and aux data in the shared base or patch.`,
  );
}

function validateBrightness(targetName, path, value) {
  if (!Number.isInteger(value) || value < 0 || value > 100) {
    throw new Error(
      `${targetName} platform override value at ${path} must be an integer from 0 through 100; received ${JSON.stringify(value)}.`,
    );
  }
}

function isAllowedPrefix(path) {
  return PLATFORM_OVERRIDE_ALLOWLIST.some(
    (allowedPath) => allowedPath.startsWith(`${path}.`),
  );
}

function readFile(root, path) {
  try {
    return readFileSync(resolve(root, path), "utf8");
  } catch (error) {
    throw new Error(`Unable to read ${path}: ${error.message}`, { cause: error });
  }
}

function readJson(root, path) {
  const content = readFile(root, path);
  try {
    return JSON.parse(content);
  } catch (error) {
    throw new Error(`Unable to parse ${path} as JSON: ${error.message}`, { cause: error });
  }
}

function deepMerge(baseValue, overrideValue) {
  if (Array.isArray(baseValue) || Array.isArray(overrideValue)) return clone(overrideValue ?? baseValue);
  if (isObject(baseValue) && isObject(overrideValue)) {
    const merged = { ...baseValue };
    for (const [key, value] of Object.entries(overrideValue)) {
      merged[key] = deepMerge(baseValue[key], value);
    }
    return merged;
  }
  return clone(overrideValue ?? baseValue);
}

function clone(value) {
  return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function stableJson(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const rootArgIndex = process.argv.indexOf("--root");
  const rootArgument = rootArgIndex >= 0 ? process.argv[rootArgIndex + 1] : undefined;
  if (rootArgIndex >= 0 && !rootArgument) {
    console.error("Config generation failed: --root requires a directory path.");
    process.exitCode = 1;
  } else {
    try {
      const root = rootArgument ? resolve(rootArgument) : defaultRoot;
      const failed = generateDefaultConfigs(root, process.argv.includes("--check"));
      if (failed) process.exitCode = 1;
    } catch (error) {
      console.error(`Config generation failed: ${error.message}`);
      process.exitCode = 1;
    }
  }
}
