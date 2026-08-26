import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";

const AUDIT = fileURLToPath(new URL("./quality-audit.mjs", import.meta.url));
const REQUIRED_ASSETS = [
  "tools/storage/octessera-sd-card",
  "tools/storage/octessera-sd-card-lib.sh",
  "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/octessera-sd-card",
  "tools/pi-image/stage4-octessera/files/root/etc/systemd/system/octessera-sd-card.service",
  "tools/pi-image/stage4-octessera/files/root/etc/udev/rules.d/99-octessera-sd-card.rules",
];
const INCLUDED_EXTENSIONS = [
  ".bash",
  ".js",
  ".mjs",
  ".ps1",
  ".psm1",
  ".py",
  ".rs",
  ".sh",
  ".ts",
  ".tsx",
];
const EXCLUDED_DIRECTORIES = [
  ".opencode",
  ".slim",
  "artifacts",
  "build",
  "generated",
  "gen",
  "hardware/enclosure/review",
  "hardware/pcb/gerber",
  "release-artifacts",
  "target",
  "third_party",
  "vendor",
];

const writeFile = (root, relativePath, contents) => {
  const file = join(root, relativePath);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, contents);
};

const sourceWithLines = (extension, count = 501, trailingNewline = true) =>
  Array.from(
    { length: count },
    (_, index) => `fixture ${extension} ${index}`,
  ).join("\n") + (trailingNewline ? "\n" : "");

const runAudit = (root) =>
  spawnSync(process.execPath, [AUDIT], {
    cwd: root,
    encoding: "utf8",
  });

test("quality audit includes owned script extensions and excludes generated artifacts", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    for (const asset of REQUIRED_ASSETS) writeFile(root, asset, "");
    for (const extension of INCLUDED_EXTENSIONS)
      writeFile(root, `src/included${extension}`, "source\n");
    writeFile(root, "src/shebang-script", "#!/bin/sh\nsource\n");
    writeFile(root, "src/no-shebang", sourceWithLines("no-shebang"));
    writeFile(root, "src/excluded.txt", sourceWithLines(".txt"));
    writeFile(root, "src/exact-500.py", sourceWithLines(".py", 500));
    writeFile(
      root,
      "src/exact-500-no-newline.sh",
      sourceWithLines(".sh", 500, false),
    );
    writeFile(
      root,
      "tools/pi-image/stage4-octessera/files/root/usr/local/sbin/deployment-script",
      sourceWithLines("deployment", 500, false),
    );
    for (const directory of EXCLUDED_DIRECTORIES)
      writeFile(root, `${directory}/excluded.py`, sourceWithLines(".py"));
    writeFile(
      root,
      "release-artifacts/artifact-script",
      "#!/bin/sh\n" + sourceWithLines("artifact"),
    );

    const passing = runAudit(root);
    assert.equal(passing.status, 0, passing.stderr);
    assert.match(passing.stdout, /Files scanned: 16/);
    assert.match(passing.stdout, /Files over enforced limit \(> 500 LOC\): 0/);
    assert.match(passing.stdout, /deployment-script: 500 LOC/);
    assert.doesNotMatch(
      passing.stdout,
      /excluded\.txt|no-shebang|artifact-script/,
    );

    writeFile(root, "src/exact-500.py", sourceWithLines(".py"));
    writeFile(
      root,
      "src/exact-500-no-newline.sh",
      sourceWithLines(".sh", 501, false),
    );
    const failing = runAudit(root);
    assert.equal(failing.status, 1, failing.stderr);
    assert.match(failing.stdout, /Files over enforced limit \(> 500 LOC\): 2/);
    assert.match(failing.stderr, /src\/exact-500\.py/);
    assert.match(failing.stderr, /src\/exact-500-no-newline\.sh/);
    assert.doesNotMatch(failing.stderr, /excluded/);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});
