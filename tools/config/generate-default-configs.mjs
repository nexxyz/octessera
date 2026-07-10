import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const check = process.argv.includes("--check");

const targets = [
  { name: "desktop", output: ["config", "generated", "desktop", "default.json"] },
  { name: "pi", output: ["config", "generated", "pi", "default.json"] },
];

const base = readJson("config/defaults/base.json");
let failed = false;

for (const target of targets) {
  const override = readJson(`config/defaults/${target.name}.json`);
  const generated = stableJson(deepMerge(base, override));
  const outputPath = resolve(root, ...target.output);
  if (check) {
    const existing = readFileSync(outputPath, "utf8");
    if (existing !== generated) {
      console.error(`${target.output.join("/")} is out of date. Run corepack pnpm run config:generate.`);
      failed = true;
    }
  } else {
    writeFileSync(outputPath, generated);
  }
}

const piDefault = readFileSync(resolve(root, "config", "generated", "pi", "default.json"), "utf8");
const canonicalPath = resolve(root, "config", "default.json");
if (check) {
  const existing = readFileSync(canonicalPath, "utf8");
  if (existing !== piDefault) {
    console.error("config/default.json is out of date. Run corepack pnpm run config:generate.");
    failed = true;
  }
} else {
  writeFileSync(canonicalPath, piDefault);
}

if (failed) process.exit(1);

function readJson(path) {
  return JSON.parse(readFileSync(resolve(root, path), "utf8"));
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
