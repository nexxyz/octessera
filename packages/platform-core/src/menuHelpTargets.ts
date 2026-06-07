import type { HelpTarget } from "./menuHelp";
import type { MenuNode } from "./index";

export function menuHelpTargetFromNode(path: string, node: MenuNode): HelpTarget {
  const nodeLabel = "label" in node ? node.label : node.kind;
  const fullPath = canonicalizeMenuPath(`${path} > ${nodeLabel}`);
  if (node.kind === "group") {
    return { path: fullPath, key: "", kind: "group", label: node.label ?? "Section" };
  }
  if (node.kind === "action") {
    const a = node.action;
    if (a.type === "behavior_action") return { path: fullPath, key: `action:behavior_action:${a.actionType}`, kind: "action", label: node.label ?? "Action" };
    if (a.type === "midi_select_output") return { path: fullPath, key: `action:midi_select_output:${a.id ?? "null"}`, kind: "action", label: node.label ?? "Action" };
    if (a.type === "midi_select_input") return { path: fullPath, key: `action:midi_select_input:${a.id ?? "null"}`, kind: "action", label: node.label ?? "Action" };
    if (a.type === "preset_load") return { path: fullPath, key: `action:preset_load:${a.name}`, kind: "action", label: node.label ?? "Action" };
    if (a.type === "preset_delete") return { path: fullPath, key: `action:preset_delete:${a.name}`, kind: "action", label: node.label ?? "Action" };
    if (a.type === "preset_rename_pick") return { path: fullPath, key: `action:preset_rename_pick:${a.name}`, kind: "action", label: node.label ?? "Action" };
    return { path: fullPath, key: `action:${a.type}`, kind: "action", label: node.label ?? "Action" };
  }
  if (node.kind === "text") return { path: fullPath, key: `key:${node.key}`, kind: "text", label: node.label ?? "Text" };
  if (node.kind === "number" || node.kind === "enum" || node.kind === "bool") {
    return { path: fullPath, key: `key:${node.key}`, kind: node.kind, label: node.label ?? "Setting" };
  }
  return { path: fullPath, key: "", kind: node.kind, label: nodeLabel ?? "Menu Entry" };
}

function canonicalizeMenuPath(path: string): string {
  return path
    .replace(/\bP\d+:\s[^>]+$/g, "P*: *")
    .replace(/\bP\d+:\s[^>]+(?=\s>)/g, "P*: *")
    .replace(/\bI\d+:\s[^>]+$/g, "Instrument 1")
    .replace(/\bI\d+:\s[^>]+(?=\s>)/g, "Instrument *")
    .replace(/\bB\d+:\s[^>]+$/g, "B*: *")
    .replace(/\bB\d+:\s[^>]+(?=\s>)/g, "B*: *");
}
