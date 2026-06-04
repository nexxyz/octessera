import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { resolve, extname, join } from "node:path";

const ROOT = resolve(process.cwd());
const INCLUDE_EXT = new Set([".ts", ".tsx"]);
const IGNORE_DIRS = new Set(["node_modules", "dist", "build", ".git", ".turbo", ".pnpm-store", "coverage"]);

const thresholds = {
  fileLocWarn: 500,
  fileLocHard: 800,
  fnLocWarn: 60,
  fnLocHard: 90,
  complexityWarn: 10,
  complexityHard: 15,
  paramsWarn: 4,
  paramsHard: 6
};

function listFiles(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      if (IGNORE_DIRS.has(entry)) continue;
      out.push(...listFiles(full));
      continue;
    }
    if (!INCLUDE_EXT.has(extname(entry))) continue;
    out.push(full);
  }
  return out;
}

function lineCount(text) {
  return text.split(/\r?\n/).length;
}

function scanFunctions(text) {
  const lines = text.split(/\r?\n/);
  const fns = [];
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const m = line.match(/^\s*(export\s+)?function\s+([A-Za-z0-9_]+)\s*\(([^)]*)\)\s*\{/);
    if (!m) continue;
    const name = m[2];
    const params = m[3].trim();
    const paramCount = params.length === 0 ? 0 : params.split(",").length;
    let depth = 0;
    let end = i;
    let body = "";
    for (let j = i; j < lines.length; j += 1) {
      const l = lines[j];
      for (const ch of l) {
        if (ch === "{") depth += 1;
        if (ch === "}") depth -= 1;
      }
      body += `${l}\n`;
      if (depth === 0 && j > i) {
        end = j;
        break;
      }
    }
    const loc = end - i + 1;
    const complexity = 1 + (body.match(/\bif\b|\bfor\b|\bwhile\b|\bcase\b|\bcatch\b|\?\s*[^:]/g) || []).length;
    fns.push({ name, start: i + 1, end: end + 1, loc, complexity, paramCount });
  }
  return fns;
}

const files = listFiles(ROOT);
const fileStats = [];
const fnStats = [];
const namingHits = [];

for (const file of files) {
  const rel = file.replace(`${ROOT}\\`, "").replace(/\\/g, "/");
  const text = readFileSync(file, "utf8");
  const loc = lineCount(text);
  fileStats.push({ file: rel, loc });
  const fns = scanFunctions(text).map((f) => ({ ...f, file: rel }));
  fnStats.push(...fns);

  const behaviour = (text.match(/\bbehaviour\b/gi) || []).length;
  if (behaviour > 0) namingHits.push({ file: rel, token: "behaviour", count: behaviour });
}

const largeFiles = fileStats.filter((f) => f.loc > thresholds.fileLocWarn).sort((a, b) => b.loc - a.loc);
const complexFns = fnStats.filter((f) => f.complexity > thresholds.complexityWarn).sort((a, b) => b.complexity - a.complexity);
const longFns = fnStats.filter((f) => f.loc > thresholds.fnLocWarn).sort((a, b) => b.loc - a.loc);
const wideFns = fnStats.filter((f) => f.paramCount > thresholds.paramsWarn).sort((a, b) => b.paramCount - a.paramCount);

const report = [];
report.push("# Code Quality Baseline");
report.push("");
report.push("## Standards (Staged Warning Mode)");
report.push(`- File LOC warn/hard: ${thresholds.fileLocWarn}/${thresholds.fileLocHard}`);
report.push(`- Function LOC warn/hard: ${thresholds.fnLocWarn}/${thresholds.fnLocHard}`);
report.push(`- Cyclomatic complexity warn/hard: ${thresholds.complexityWarn}/${thresholds.complexityHard}`);
report.push(`- Function params warn/hard: ${thresholds.paramsWarn}/${thresholds.paramsHard}`);
report.push("");
report.push("## Summary");
report.push(`- Files scanned: ${fileStats.length}`);
report.push(`- Functions scanned (named function declarations): ${fnStats.length}`);
report.push(`- Large files (> ${thresholds.fileLocWarn} LOC): ${largeFiles.length}`);
report.push(`- Complex functions (> ${thresholds.complexityWarn}): ${complexFns.length}`);
report.push(`- Long functions (> ${thresholds.fnLocWarn} LOC): ${longFns.length}`);
report.push(`- Wide signatures (> ${thresholds.paramsWarn} params): ${wideFns.length}`);
report.push("");

report.push("## Top Large Files");
for (const f of largeFiles.slice(0, 20)) report.push(`- ${f.file}: ${f.loc} LOC`);
report.push("");

report.push("## Top Complex Functions");
for (const f of complexFns.slice(0, 20)) report.push(`- ${f.file}:${f.start} ${f.name}() complexity=${f.complexity}, loc=${f.loc}`);
report.push("");

report.push("## Top Long Functions");
for (const f of longFns.slice(0, 20)) report.push(`- ${f.file}:${f.start} ${f.name}() loc=${f.loc}, complexity=${f.complexity}`);
report.push("");

report.push("## Naming Consistency (behavior vs behaviour)");
if (namingHits.length === 0) {
  report.push("- No `behaviour` identifier tokens found.");
} else {
  for (const n of namingHits) report.push(`- ${n.file}: ${n.count} occurrences of 'behaviour'`);
}
report.push("");

const outPath = resolve(ROOT, "docs", "code-quality-baseline.md");
writeFileSync(outPath, `${report.join("\n")}\n`, "utf8");
console.log(`Wrote baseline report: ${outPath}`);
