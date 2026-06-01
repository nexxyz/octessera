import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

const SRC = resolve(process.cwd(), "src");
const thresholds = {
  fileLocMax: 500,
  fnLocWarn: 60,
  complexityWarn: 10,
  paramsWarn: 4
};

function listTsFiles(dir: string): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      out.push(...listTsFiles(full));
      continue;
    }
    if (!entry.endsWith(".ts") && !entry.endsWith(".tsx")) continue;
    out.push(full);
  }
  return out;
}

function scanFunctions(text: string): Array<{ name: string; loc: number; complexity: number; params: number; line: number }> {
  const lines = text.split(/\r?\n/);
  const out: Array<{ name: string; loc: number; complexity: number; params: number; line: number }> = [];
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
    out.push({ name, loc, complexity, params: paramCount, line: i + 1 });
  }
  return out;
}

const files = listTsFiles(SRC);
let warnings = 0;
let errors = 0;

for (const file of files) {
  const text = readFileSync(file, "utf8");
  const lines = text.split(/\r?\n/).length;
  const isGenerated = /\.generated\.(ts|tsx)$/.test(file);
  if (!isGenerated && lines > thresholds.fileLocMax) {
    errors += 1;
    console.error(`[quality] file-loc ${file} ${lines} > ${thresholds.fileLocMax}`);
  }
  for (const fn of scanFunctions(text)) {
    if (fn.loc > thresholds.fnLocWarn) {
      warnings += 1;
      console.warn(`[quality] fn-loc ${file}:${fn.line} ${fn.name} ${fn.loc} > ${thresholds.fnLocWarn}`);
    }
    if (fn.complexity > thresholds.complexityWarn) {
      warnings += 1;
      console.warn(`[quality] fn-complexity ${file}:${fn.line} ${fn.name} ${fn.complexity} > ${thresholds.complexityWarn}`);
    }
    if (fn.params > thresholds.paramsWarn) {
      warnings += 1;
      console.warn(`[quality] fn-params ${file}:${fn.line} ${fn.name} ${fn.params} > ${thresholds.paramsWarn}`);
    }
  }
}

if (errors > 0) {
  console.error(`[quality] staged checks failed with ${errors} error(s)`);
  process.exit(1);
}

if (warnings === 0) {
  console.log("[quality] staged checks passed with no warnings");
} else {
  console.log(`[quality] staged checks emitted ${warnings} warning(s)`);
}
