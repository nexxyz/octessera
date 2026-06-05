import type { ActionSpec, AuxTurnBinding, MenuNode, PlatformState } from "./index";
import { paramBindingFromMenuNode } from "./paramMod";

let cachedSkeleton: MenuNode[] | null = null;

export function invalidateParamSkeleton(): void {
  cachedSkeleton = null;
}

function extractLeaves(nodes: MenuNode[]): MenuNode[] {
  const result: MenuNode[] = [];
  for (const node of nodes) {
    if (node.kind === "group") {
      if (typeof node.children === "function") {
        const origFn = node.children;
        result.push({
          ...node,
          children: (state: PlatformState<any>) => extractLeaves(origFn(state))
        });
      } else {
        const filtered = extractLeaves(node.children);
        if (filtered.length > 0) result.push({ ...node, children: filtered });
      }
    } else if (node.kind === "number" || node.kind === "enum" || node.kind === "bool") {
      result.push(node);
    }
  }
  return result;
}

function isAssignable(node: MenuNode): boolean {
  return node.kind === "number" || node.kind === "enum" || node.kind === "bool";
}

function wrapNodes(nodes: MenuNode[], onSelect: (binding: AuxTurnBinding) => ActionSpec): MenuNode[] {
  const result: MenuNode[] = [];
  for (const node of nodes) {
    if (node.kind === "group") {
      if (typeof node.children === "function") {
        const origFn = node.children;
        result.push({
          ...node,
          children: (state: PlatformState<any>) => wrapNodes(origFn(state), onSelect)
        });
      } else {
        const children = wrapNodes(node.children, onSelect);
        if (children.length > 0) result.push({ ...node, children });
      }
    } else if (isAssignable(node)) {
      const binding = paramBindingFromMenuNode(node);
      if (binding) {
        result.push({ kind: "action" as const, label: (node as any).label, action: onSelect(binding) });
      }
    }
  }
  return result;
}

export function buildParamSkeleton(rootChildren?: MenuNode[]): MenuNode[] {
  if (cachedSkeleton) return cachedSkeleton;
  if (rootChildren) cachedSkeleton = extractLeaves(rootChildren);
  return cachedSkeleton ?? [];
}

export function withParamActions(
  skeleton: MenuNode[],
  onSelect: (binding: AuxTurnBinding) => ActionSpec
): MenuNode[] {
  return wrapNodes(skeleton, onSelect);
}
