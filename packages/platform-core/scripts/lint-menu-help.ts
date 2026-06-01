import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createInitialState, enumerateEnumHelpTargets, enumerateMenuHelpTargets } from "../src/index";
import { lifeBehavior } from "@cellsymphony/behaviors-life";

type Entry = { id: string; path: string; key: string; kind: string; title: string; line1: string; line2: string };

const tsvPath = resolve(process.cwd(), "resources", "menu-help-texts.tsv");
const raw = readFileSync(tsvPath, "utf8");
const lines = raw.split(/\r?\n/).filter((l) => l.trim().length > 0 && !l.trim().startsWith("#"));

const expectedHeader = ["id", "path", "key", "kind", "title", "line1", "line2"];
if (lines.length === 0) throw new Error("menu-help-texts.tsv is empty");
const header = lines[0].split("\t");
if (header.join("\t") !== expectedHeader.join("\t")) {
  throw new Error(`Invalid header in menu-help-texts.tsv. Expected: ${expectedHeader.join("\t")}`);
}

const entries: Entry[] = [];
for (let i = 1; i < lines.length; i += 1) {
  const cols = lines[i].split("\t");
  if (cols.length < 7) throw new Error(`Invalid row ${i + 1}: expected 7 columns`);
  const [id, path, key, kind, title, line1, line2] = cols;
  if (!id || !kind || !line1) throw new Error(`Invalid row ${i + 1}: id/kind/line1 are required`);
  entries.push({ id, path, key, kind, title, line1, line2 });
}

function globMatch(pattern: string, value: string): boolean {
  if (!pattern || pattern === "*") return true;
  if (!pattern.includes("*")) return pattern === value;
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^>]*");
  return new RegExp(`^${escaped}$`).test(value);
}

function matches(entry: Entry, target: { path: string; key: string; kind: string }): boolean {
  if (entry.kind && entry.kind !== "*" && entry.kind !== target.kind) return false;
  if (entry.key) {
    if (!globMatch(entry.key, target.key)) return false;
  } else if (entry.path && !globMatch(entry.path, target.path)) {
    return false;
  }
  return true;
}

function tier(entry: Entry, target: { path: string; key: string; kind: string }): number {
  if (!matches(entry, target)) return -1;
  if (entry.key && !entry.key.includes("*")) return 0;
  if (entry.key && entry.key.includes("*")) {
    if (entry.key === "action:*" || entry.key === "key:*") return 4;
    return 1;
  }
  if (entry.path && !entry.path.includes("*")) return 2;
  if (entry.path && entry.path.includes("*")) return 3;
  return 4;
}

function isExplicit(entry: Entry): boolean {
  const key = entry.key.trim();
  const path = entry.path.trim();
  return key.length > 0 || (path.length > 0 && path !== "*");
}

const state = createInitialState(lifeBehavior);
state.system.oledMode = "normal";
state.system.presetNames = ["Preset A"];
state.system.selectedPreset = "Preset A";
state.system.currentPresetName = "Preset A";
state.system.midiOutputs = [{ id: "out-1", name: "Out Port" }];
state.system.midiInputs = [{ id: "in-1", name: "In Port" }];

const targets = enumerateMenuHelpTargets(state);
const enumTargets = enumerateEnumHelpTargets(state);
const misses: typeof targets = [];
const ambiguities: Array<{ target: (typeof targets)[number]; ids: string[] }> = [];
const buckets = { exactKey: 0, wildcardKey: 0, exactPath: 0, wildcardPath: 0, kindFallback: 0 };

for (const t of targets) {
  const matched = entries.filter((e) => isExplicit(e) && matches(e, t));
  if (matched.length === 0) {
    misses.push(t);
    continue;
  }
  let bestTier = 99;
  for (const m of matched) {
    const x = tier(m, t);
    if (x >= 0 && x < bestTier) bestTier = x;
  }
  const best = matched.filter((m) => tier(m, t) === bestTier);
  if (best.length > 1) {
    ambiguities.push({ target: t, ids: best.map((b) => b.id) });
  }
  if (bestTier === 0) buckets.exactKey += 1;
  else if (bestTier === 1) buckets.wildcardKey += 1;
  else if (bestTier === 2) buckets.exactPath += 1;
  else if (bestTier === 3) buckets.wildcardPath += 1;
  else buckets.kindFallback += 1;
}

const enumHelpErrors: string[] = [];
const seenEnumCanonical = new Set<string>();

function enumCanonicalKey(key: string): string {
  return key.replace(/instruments\.\d+\./g, "instruments.*.");
}

function shouldEnforceEnumHelp(target: { key: string; options: string[] }): boolean {
  const key = enumCanonicalKey(target.key);
  if (key === "key:scanMode") return true;
  if (key === "key:activeBehavior") return true;
  if (key === "key:instruments.*.type") return true;
  return false;
}

for (const t of enumTargets) {
  const canonicalKey = enumCanonicalKey(t.key);
  if (seenEnumCanonical.has(canonicalKey)) continue;
  seenEnumCanonical.add(canonicalKey);
  if (!shouldEnforceEnumHelp(t)) continue;
  const matched = entries.filter((e) => isExplicit(e) && matches(e, t));
  if (matched.length === 0) continue;
  let bestTier = 99;
  for (const m of matched) {
    const x = tier(m, t);
    if (x >= 0 && x < bestTier) bestTier = x;
  }
  const best = matched.filter((m) => tier(m, t) === bestTier)[0];
  if (!best) continue;
  const text = `${best.line1} ${best.line2}`.toLowerCase();
  const missing: string[] = [];
  for (const option of t.options) {
    const token = option.toLowerCase();
    const normalized = token.replaceAll("_", " ");
    const aliases = new Set<string>([token, normalized]);
    if (t.key === "key:scanMode" && token === "immediate") aliases.add("none");
    let found = false;
    for (const a of aliases) {
      if (text.includes(a)) {
        found = true;
        break;
      }
    }
    if (!found) missing.push(option);
  }
  if (missing.length > 0) {
    enumHelpErrors.push(`- key=${t.key} path=${t.path} missing options=${missing.join(",")}`);
  }
}

if (enumHelpErrors.length > 0) {
  console.error("Enum help entries must describe all current options:");
  for (const err of enumHelpErrors) console.error(err);
  process.exit(1);
}

if (misses.length > 0) {
  console.error("Missing menu help entries:");
  for (const m of misses) {
    console.error(`- kind=${m.kind} key=${m.key || "(none)"} path=${m.path}`);
  }
  process.exit(1);
}

if (ambiguities.length > 0) {
  console.error("Ambiguous menu help matches:");
  for (const a of ambiguities) {
    console.error(`- kind=${a.target.kind} key=${a.target.key || "(none)"} path=${a.target.path} ids=${a.ids.join(",")}`);
  }
  process.exit(1);
}

console.log(
  `menu-help lint passed (${targets.length} covered: exactKey=${buckets.exactKey}, wildcardKey=${buckets.wildcardKey}, exactPath=${buckets.exactPath}, wildcardPath=${buckets.wildcardPath}, kindFallback=${buckets.kindFallback})`
);

const fallbackRatio = targets.length === 0 ? 0 : buckets.kindFallback / targets.length;
if (fallbackRatio > 0.4) {
  console.warn(
    `menu-help lint warning: ${Math.round(fallbackRatio * 100)}% of entries resolve via kind fallback. Consider adding more specific key/path rows.`
  );
}
