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

const withRequiredAssets = (root) => {
  for (const asset of REQUIRED_ASSETS) writeFile(root, asset, "");
};

test("quality audit includes owned script extensions and excludes generated artifacts", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    withRequiredAssets(root);
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

test("Rust question marks do not enter the JavaScript/TypeScript complexity inventory", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    withRequiredAssets(root);
    writeFile(
      root,
      "src/rust-question-marks.rs",
      [
        "fn rust_question_marks(value: usize) {",
        ...Array.from({ length: 11 }, () => "  let _value = value?;"),
        "}",
      ].join("\n"),
    );

    const result = runAudit(root);
    assert.equal(result.status, 0, result.stderr);
    assert.match(
      result.stdout,
      /JavaScript\/TypeScript functions over enforced complexity limit \(> 10\): 0/,
    );
    assert.doesNotMatch(result.stdout, /rust-question-marks\.rs:.*complexity=/);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("quality audit prints every JavaScript/TypeScript complexity finding", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    withRequiredAssets(root);
    const source = Array.from(
      { length: 21 },
      (_, index) =>
        `function complex${index}(value) { ${Array.from(
          { length: 10 },
          () => "if (value) {}",
        ).join(" ")} }`,
    ).join("\n");
    writeFile(root, "src/many-complex-functions.js", source);

    const result = runAudit(root);
    assert.equal(result.status, 1, result.stderr);
    assert.match(
      result.stdout,
      /JavaScript\/TypeScript functions over enforced complexity limit \(> 10\): 21/,
    );
    assert.equal(
      (result.stdout.match(/many-complex-functions\.js:.*complexity=11/g) || [])
        .length,
      21,
    );
    assert.match(result.stdout, /many-complex-functions\.js:1 complex0\(\)/);
    assert.match(result.stdout, /many-complex-functions\.js:21 complex20\(\)/);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("quality audit reports complexity and file-length failures together", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    withRequiredAssets(root);
    writeFile(
      root,
      "src/complex.js",
      `function complex(value) { ${Array.from(
        { length: 10 },
        () => "if (value) {}",
      ).join(" ")} }\n`,
    );
    writeFile(root, "src/too-long.py", sourceWithLines(".py"));

    const result = runAudit(root);
    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      /Quality audit failed: 1 JavaScript\/TypeScript function\(s\) exceed the enforced > 10 complexity limit\./,
    );
    assert.match(
      result.stderr,
      /Quality audit failed: 1 file\(s\) exceed the enforced > 500 LOC limit\./,
    );
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});

test("JavaScript/TypeScript AST parse failure is fatal and file-specific", () => {
  const root = mkdtempSync(join(tmpdir(), "octessera-quality-audit-"));
  try {
    withRequiredAssets(root);
    writeFile(root, "src/broken.ts", "function broken() { return ???; }\n");

    const result = runAudit(root);
    assert.equal(result.status, 1);
    assert.match(
      result.stderr,
      /Quality audit failed: Babel AST parse failed for JavaScript\/TypeScript file src\/broken\.ts:/,
    );
    assert.doesNotMatch(result.stderr, /approximate JavaScript regex scanner/);
  } finally {
    rmSync(root, { force: true, recursive: true });
  }
});
