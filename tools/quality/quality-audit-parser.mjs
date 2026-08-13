import { parse } from "@babel/parser";

const FUNCTION_NODE_TYPES = new Set([
  "FunctionDeclaration",
  "FunctionExpression",
  "ArrowFunctionExpression",
  "ObjectMethod",
  "ClassMethod",
  "ClassPrivateMethod",
]);

const COMPLEXITY_NODE_TYPES = new Set([
  "IfStatement",
  "ConditionalExpression",
  "ForStatement",
  "ForInStatement",
  "ForOfStatement",
  "WhileStatement",
  "DoWhileStatement",
  "CatchClause",
  "SwitchCase",
]);

const JS_EXTENSIONS = new Set([".js", ".mjs", ".ts", ".tsx"]);
const LOGICAL_COMPLEXITY_OPERATORS = new Set(["&&", "||", "??"]);

function isNode(value) {
  return (
    value !== null &&
    typeof value === "object" &&
    typeof value.type === "string"
  );
}

function forEachChild(node, visit) {
  for (const [key, value] of Object.entries(node)) {
    if (
      key === "loc" ||
      key === "start" ||
      key === "end" ||
      key === "tokens" ||
      key === "comments"
    )
      continue;
    if (Array.isArray(value)) {
      for (const child of value) {
        if (isNode(child)) visit(child);
      }
    } else if (isNode(value)) {
      visit(value);
    }
  }
}

function walk(node, visit, parent = null) {
  visit(node, parent);
  forEachChild(node, (child) => walk(child, visit, node));
}

function propertyName(node) {
  if (!node) return null;
  if (node.type === "Identifier") return node.name;
  if (node.type === "PrivateName") return `#${propertyName(node.id)}`;
  if (node.type === "StringLiteral" || node.type === "NumericLiteral")
    return String(node.value);
  return "computed";
}

function bindingName(node) {
  return node?.type === "Identifier" ? node.name : null;
}

function functionName(node, parent) {
  if (node.id?.type === "Identifier") return node.id.name;
  if (
    node.type === "ObjectMethod" ||
    node.type === "ClassMethod" ||
    node.type === "ClassPrivateMethod"
  ) {
    const name = propertyName(node.key) ?? "anonymous";
    return node.kind === "get" || node.kind === "set"
      ? `${node.kind} ${name}`
      : name;
  }
  if (parent?.type === "VariableDeclarator" && parent.init === node)
    return bindingName(parent.id) ?? "anonymous";
  if (parent?.type === "AssignmentExpression" && parent.right === node) {
    return (
      bindingName(parent.left) ??
      propertyName(parent.left?.property) ??
      "anonymous"
    );
  }
  if (parent?.type === "ObjectProperty" && parent.value === node)
    return propertyName(parent.key) ?? "anonymous";
  return "anonymous";
}

function countComplexity(node) {
  let complexity = 1;
  const visit = (current) => {
    if (current !== node && FUNCTION_NODE_TYPES.has(current.type)) return;
    if (
      COMPLEXITY_NODE_TYPES.has(current.type) &&
      (current.type !== "SwitchCase" ||
        (current.test !== null && current.test !== undefined))
    ) {
      complexity += 1;
    }
    if (
      current.type === "LogicalExpression" &&
      LOGICAL_COMPLEXITY_OPERATORS.has(current.operator)
    )
      complexity += 1;
    forEachChild(current, visit);
  };

  visit(node.body);
  return complexity;
}

function functionStats(node, parent) {
  return {
    name: functionName(node, parent),
    start: node.loc.start.line,
    end: node.loc.end.line,
    loc: node.loc.end.line - node.loc.start.line + 1,
    complexity: countComplexity(node),
    paramCount: node.params.length,
  };
}

function parserPlugins(extension) {
  if (extension === ".js" || extension === ".mjs") return [];
  if (extension === ".tsx") return ["jsx", ["typescript", { isTSX: true }]];
  if (extension === ".ts") return ["typescript"];
  throw new Error(`Unsupported JavaScript extension: ${extension}`);
}

export function scanJavaScriptFunctions(text, extension) {
  if (!JS_EXTENSIONS.has(extension))
    throw new Error(`Unsupported JavaScript extension: ${extension}`);
  const ast = parse(text, {
    sourceType: "unambiguous",
    plugins: parserPlugins(extension),
  });
  const functions = [];
  walk(ast, (node, parent) => {
    if (FUNCTION_NODE_TYPES.has(node.type))
      functions.push(functionStats(node, parent));
  });
  return functions;
}

function scanRegexFunctions(text, extension) {
  const lines = text.split(/\r?\n/);
  const fns = [];
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const match =
      extension === ".rs"
        ? line.match(
            /^\s*(pub(\([^)]*\))?\s+)?fn\s+([A-Za-z0-9_]+)\s*\(([^)]*)\)/,
          )
        : line.match(
            /^\s*(export\s+)?function\s+([A-Za-z0-9_]+)\s*\(([^)]*)\)\s*\{/,
          );
    if (!match) continue;
    const name = extension === ".rs" ? match[3] : match[2];
    const params = (extension === ".rs" ? match[4] : match[3]).trim();
    const paramCount = params.length === 0 ? 0 : params.split(",").length;
    let depth = 0;
    let end = i;
    let body = "";
    for (let j = i; j < lines.length; j += 1) {
      const currentLine = lines[j];
      for (const ch of currentLine) {
        if (ch === "{") depth += 1;
        if (ch === "}") depth -= 1;
      }
      body += `${currentLine}\n`;
      if (depth === 0 && j > i) {
        end = j;
        break;
      }
    }
    const loc = end - i + 1;
    const complexity =
      1 +
      (
        body.match(/\bif\b|\bfor\b|\bwhile\b|\bcase\b|\bcatch\b|\?\s*[^:]/g) ||
        []
      ).length;
    fns.push({ name, start: i + 1, end: end + 1, loc, complexity, paramCount });
  }
  return fns;
}

export function scanApproximateJavaScriptFunctions(text) {
  return scanRegexFunctions(text, ".mjs");
}

export function scanApproximateRustFunctions(text) {
  return scanRegexFunctions(text, ".rs");
}

export function scanJavaScriptFunctionsWithFallback(
  text,
  extension,
  onParseError = console.warn,
) {
  try {
    return scanJavaScriptFunctions(text, extension);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    onParseError(
      `Quality audit warning: Babel parse failed; using approximate JavaScript regex scanner. ${message}`,
    );
    return scanApproximateJavaScriptFunctions(text);
  }
}
