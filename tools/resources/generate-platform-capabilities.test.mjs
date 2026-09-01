import { equal, match } from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { test } from "node:test";

const scriptPath = fileURLToPath(import.meta.url);
const generatorPath = join(dirname(scriptPath), "generate-platform-capabilities.mjs");
const repositoryRoot = resolve(dirname(scriptPath), "..", "..");
const canonicalCapabilities = JSON.parse(
  readFileSync(join(repositoryRoot, "resources", "platform-capabilities.json"), "utf8"),
);

test("generates physical voice-lane capacities in the TypeScript contract", () => {
  const result = runGenerator();
  equal(result.status, 0);
  match(result.generated, /"synthVoiceLaneCapacity": 64/);
  match(result.generated, /"sampleVoiceLaneCapacity": 64/);
});

test("rejects a global voice policy above its physical lane capacity", () => {
  const result = runGenerator({ synthVoiceLaneCapacity: 8, maxSynthVoices: 9 });
  equal(result.status, 1);
  match(result.stderr, /maxSynthVoices.*physical lane capacity 'synthVoiceLaneCapacity'/);
});

test("rejects a per-slot voice policy above its physical lane capacity", () => {
  const result = runGenerator({
    sampleVoiceLaneCapacity: 8,
    maxSampleVoices: 8,
    maxSampleVoicesPerSlot: 9,
  });
  equal(result.status, 1);
  match(result.stderr, /maxSampleVoicesPerSlot.*physical lane capacity 'sampleVoiceLaneCapacity'/);
});

test("rejects a nonpositive physical voice-lane capacity", () => {
  const result = runGenerator({ synthVoiceLaneCapacity: 0 });
  equal(result.status, 1);
  match(result.stderr, /synthVoiceLaneCapacity.*expected positive integer/);
});

function runGenerator(overrides = {}) {
  const root = mkdtempSync(join(tmpdir(), "octessera-platform-capabilities-"));
  try {
    mkdirSync(join(root, "resources"), { recursive: true });
    mkdirSync(join(root, "packages", "device-contracts", "src"), { recursive: true });
    writeFileSync(
      join(root, "resources", "platform-capabilities.json"),
      `${JSON.stringify({ ...canonicalCapabilities, ...overrides }, null, 2)}\n`,
    );
    const result = spawnSync(process.execPath, [generatorPath, "--root", root], {
      encoding: "utf8",
    });
    const generatedPath = join(root, "packages", "device-contracts", "src", "platformCapabilities.generated.ts");
    return {
      ...result,
      generated: result.status === 0 ? readFileSync(generatedPath, "utf8") : "",
    };
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}
