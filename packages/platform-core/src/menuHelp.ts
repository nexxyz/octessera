import type { MenuHelpEntry } from "./menuHelp.generated";
import { MENU_HELP_ENTRIES } from "./menuHelp.generated";

export type HelpTarget = {
  path: string;
  key: string;
  kind: string;
  label: string;
};

export type ResolvedHelp = {
  title: string;
  detail: string;
};

type MatchTier = 0 | 1 | 2 | 3 | 4 | -1;

function matchTier(entry: MenuHelpEntry, target: HelpTarget): MatchTier {
  const key = entry.key.trim();
  const path = entry.path.trim();
  const kind = entry.kind.trim();

  if (kind && kind !== "*" && kind !== target.kind) return -1;
  if (key) {
    if (!globMatch(key, target.key)) return -1;
  } else if (path && !pathMatch(path, target.path)) {
    return -1;
  }

  if (key && !key.includes("*")) return 0;
  if (key && key.includes("*")) {
    if (key === "action:*" || key === "key:*") return 4;
    return 1;
  }
  if (path && !path.includes("*")) return 2;
  if (path && path.includes("*")) return 3;
  return 4;
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
  const segments = normalizedValue.split(" > ");
  for (let i = 1; i < segments.length; i += 1) {
    if (globMatch(normalizedPattern, segments.slice(i).join(" > "))) return true;
  }
  return false;
}

function score(entry: MenuHelpEntry, target: HelpTarget): number {
  const tier = matchTier(entry, target);
  if (tier < 0) return -1;
  const keySpecificity = (entry.key ?? "").replace(/\*/g, "").length;
  const pathSpecificity = (entry.path ?? "").replace(/\*/g, "").length;
  const specificity = keySpecificity + pathSpecificity;
  const kindBonus = entry.kind && entry.kind !== "*" ? 1 : 0;
  return (500 - tier * 100) + specificity + kindBonus;
}

export function resolveMenuHelp(target: HelpTarget): ResolvedHelp {
  const best = resolveMenuHelpEntry(target);
  if (!best) {
    return {
      title: target.label || "Help",
      detail: "No help text is available for this entry yet."
    };
  }
  const lines = [best.line1, best.line2].filter((v) => v && v.trim().length > 0);
  return {
    title: best.title || target.label || "Help",
    detail: lines.join(" ")
  };
}

export function listMenuHelpEntries(): MenuHelpEntry[] {
  return MENU_HELP_ENTRIES.slice();
}

export function resolveMenuHelpEntry(target: HelpTarget): MenuHelpEntry | null {
  let best: MenuHelpEntry | null = null;
  let bestScore = -1;
  for (const entry of MENU_HELP_ENTRIES) {
    const s = score(entry, target);
    if (s > bestScore) {
      best = entry;
      bestScore = s;
    }
  }
  return bestScore < 0 ? null : best;
}
