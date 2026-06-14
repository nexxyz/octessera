import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createInitialState, enumerateEnumHelpTargets, enumerateMenuHelpTargets } from "../src/index";
import { lifeBehavior } from "@cellsymphony/behaviors-life";
import { isSpecificMenuHelpEntry, resolveMenuHelpEntry } from "../src/menuHelp";

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

function pathMatch(pattern: string, value: string): boolean {
  if (globMatch(pattern, value)) return true;
  const normalizedPattern = pattern.replace(/^Menu > /, "");
  const normalizedValue = value.replace(/^Menu > /, "");
  if (globMatch(normalizedPattern, normalizedValue)) return true;
  const segments = normalizedValue.split(" > ");
  for (let i = 1; i < segments.length; i += 1) {
    if (globMatch(normalizedPattern, segments.slice(i).join(" > "))) return true;
  }
  return false;
}

function matches(entry: Entry, target: { path: string; key: string; kind: string }): boolean {
  if (entry.kind && entry.kind !== "*" && entry.kind !== target.kind) return false;
  if (entry.key) {
    if (!globMatch(entry.key, target.key)) return false;
  } else if (entry.path && !pathMatch(entry.path, target.path)) {
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

const weakRows: Entry[] = entries.filter((entry) => !isSpecificMenuHelpEntry(entry as any));

function buildState(variant: "base" | "sampler" | "midi" | "dance-fx" | "dance-trigger-gate" | "dance-xy"): any {
  const state = createInitialState(lifeBehavior);
  state.system.oledMode = "normal";
  state.system.presetNames = ["Preset A"];
  state.system.selectedPreset = "Preset A";
  state.system.currentPresetName = "Preset A";
  state.system.midiOutputs = [{ id: "out-1", name: "Out Port" }];
  state.system.midiInputs = [{ id: "in-1", name: "In Port" }];
  state.runtimeConfig.mixer.buses[0].name = "fx";
  state.runtimeConfig.instruments[0].name = "synth";
  if (variant === "sampler") state.runtimeConfig.instruments[0].type = "sampler";
  if (variant === "midi") state.runtimeConfig.instruments[0].type = "midi";
  if (variant === "dance-fx") state.runtimeConfig.danceMode = "fx";
  if (variant === "dance-trigger-gate") state.runtimeConfig.danceMode = "trigger-gate";
  if (variant === "dance-xy") state.runtimeConfig.danceMode = "xy";
  return state;
}

const misses: Array<{ path: string; key: string; kind: string }> = [];
const ambiguities: Array<{ target: { path: string; key: string; kind: string }; ids: string[] }> = [];
const buckets = { exactKey: 0, wildcardKey: 0, exactPath: 0, wildcardPath: 0, kindFallback: 0 };
const seenTargets = new Set<string>();
let targetCount = 0;
const baseState = buildState("base");

const extraTargets = [
  { path: "Menu > L3: Voice > Instruments > Instrument * > Choose Sample", key: "", kind: "group" },
  { path: "Menu > L3: Voice > Instruments > Instrument * > Assign", key: "action:sample_assign_enter", kind: "action" },
  { path: "Menu > L3: Voice > Instruments > Instrument * > Velocity", key: "key:instruments.0.midiEngine.velocity", kind: "number" },
  { path: "Menu > L3: Voice > Instruments > Instrument * > Duration", key: "key:instruments.0.midiEngine.durationMs", kind: "number" },
  { path: "Menu > L4: Dance > Mode Grid", key: "", kind: "group" },
  { path: "Menu > L4: Dance > X Axis", key: "", kind: "group" },
  { path: "Menu > L4: Dance > Y Axis", key: "", kind: "group" },
  { path: "Menu > L4: Dance > FX Type", key: "key:dance.fx.type", kind: "enum" },
  { path: "Menu > L4: Dance > Map to Grid", key: "action:fx_assign_enter", kind: "action" }
] as const;

function inspectTarget(t: { path: string; key: string; kind: string }): void {
  const uniqueId = `${t.kind}|${t.key}|${t.path}`;
  if (seenTargets.has(uniqueId)) return;
  seenTargets.add(uniqueId);
  targetCount += 1;
  const matched = entries.filter((e) => isExplicit(e) && matches(e, t));
  const best = resolveMenuHelpEntry(t);
  if (!best) {
    misses.push(t);
    return;
  }
  const matchedSpecific = matched.filter((entry) => isSpecificMenuHelpEntry(entry as any));
  if (matchedSpecific.length === 0) {
    misses.push(t);
    return;
  }
  let bestTier = 99;
  for (const m of matchedSpecific) {
    const x = tier(m, t);
    if (x >= 0 && x < bestTier) bestTier = x;
  }
  const bestMatches = matchedSpecific.filter((m) => tier(m, t) === bestTier);
  if (bestMatches.length > 1) {
    ambiguities.push({ target: t, ids: bestMatches.map((b) => b.id) });
  }
  if (!bestMatches.some((entry) => entry.id === best.id)) {
    ambiguities.push({ target: t, ids: [best.id, ...bestMatches.map((entry) => entry.id)] });
  }
  if (bestTier === 0) buckets.exactKey += 1;
  else if (bestTier === 1) buckets.wildcardKey += 1;
  else if (bestTier === 2) buckets.exactPath += 1;
  else if (bestTier === 3) buckets.wildcardPath += 1;
  else buckets.kindFallback += 1;
}

for (const target of enumerateMenuHelpTargets(baseState)) inspectTarget(target);
for (const target of extraTargets) inspectTarget(target);

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

const seenEnumTargets = new Set<string>();
for (const t of enumerateEnumHelpTargets(baseState)) {
  const enumId = `${t.key}|${t.path}`;
  if (seenEnumTargets.has(enumId)) continue;
  seenEnumTargets.add(enumId);
  const canonicalKey = enumCanonicalKey(t.key);
  if (seenEnumCanonical.has(canonicalKey)) continue;
  seenEnumCanonical.add(canonicalKey);
  if (!shouldEnforceEnumHelp(t)) continue;
  const best = resolveMenuHelpEntry(t);
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

if (weakRows.length > 0) {
  console.error("Generic or fallback-style menu help rows are not allowed:");
  for (const row of weakRows) {
    console.error(`- id=${row.id} kind=${row.kind} path=${row.path || "(none)"} key=${row.key || "(none)"}`);
  }
  process.exit(1);
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
  `menu-help lint passed (${targetCount} covered: exactKey=${buckets.exactKey}, wildcardKey=${buckets.wildcardKey}, exactPath=${buckets.exactPath}, wildcardPath=${buckets.wildcardPath}, kindFallback=${buckets.kindFallback})`
);

const fallbackRatio = targetCount === 0 ? 0 : buckets.kindFallback / targetCount;
if (buckets.kindFallback > 0) {
  console.error(
    `menu-help lint failed: ${buckets.kindFallback} entries resolve via kind fallback (${Math.round(fallbackRatio * 100)}%). Add specific key/path rows.`
  );
  process.exit(1);
}
