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

function isAstMetadataKey(key) {
  return (
    key === "loc" ||
    key === "start" ||
    key === "end" ||
    key === "tokens" ||
    key === "comments"
  );
}

function visitChildValue(value, visit) {
  if (Array.isArray(value)) {
    for (const child of value) {
      if (isNode(child)) visit(child);
    }
  } else if (isNode(value)) {
    visit(value);
  }
}

function forEachChild(node, visit) {
  for (const [key, value] of Object.entries(node)) {
    if (isAstMetadataKey(key)) continue;
    visitChildValue(value, visit);
  }
}

function walk(node, visit, parent = null) {
  visit(node, parent);
  forEachChild(node, (child) => walk(child, visit, node));
}

function propertyName(node, fallback = null) {
  if (!node) return fallback;
  if (node.type === "Identifier") return node.name;
  if (node.type === "PrivateName") return `#${propertyName(node.id)}`;
  if (node.type === "StringLiteral" || node.type === "NumericLiteral")
    return String(node.value);
  return "computed";
}

function bindingName(node) {
  return node?.type === "Identifier" ? node.name : null;
}

function methodFunctionName(node) {
  if (
    node.type !== "ObjectMethod" &&
    node.type !== "ClassMethod" &&
    node.type !== "ClassPrivateMethod"
  )
    return null;
  const name = propertyName(node.key, "anonymous");
  if (node.kind === "get") return `get ${name}`;
  if (node.kind === "set") return `set ${name}`;
  return name;
}

function contextualFunctionName(node, parent) {
  if (parent?.type === "VariableDeclarator" && parent.init === node)
    return bindingName(parent.id) ?? "anonymous";
  if (parent?.type === "AssignmentExpression" && parent.right === node) {
    const name = bindingName(parent.left);
    if (name !== null) return name;
    return propertyName(parent.left?.property, "anonymous");
  }
  if (parent?.type === "ObjectProperty" && parent.value === node)
    return propertyName(parent.key, "anonymous");
  return "anonymous";
}

function functionName(node, parent) {
  if (node.id?.type === "Identifier") return node.id.name;
  const methodName = methodFunctionName(node);
  if (methodName !== null) return methodName;
  return contextualFunctionName(node, parent);
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

function countRustParameters(params) {
  const trimmed = params.trim();
  return trimmed.length === 0 ? 0 : trimmed.split(",").length;
}

function scanRustFunctionEnd(lines, start) {
  let depth = 0;
  let end = start;
  for (let lineIndex = start; lineIndex < lines.length; lineIndex += 1) {
    const currentLine = lines[lineIndex];
    for (const ch of currentLine) {
      if (ch === "{") depth += 1;
      if (ch === "}") depth -= 1;
    }
    if (depth === 0 && lineIndex > start) {
      end = lineIndex;
      break;
    }
  }
  return end;
}

export function scanApproximateRustFunctions(text) {
  const lines = text.split(/\r?\n/);
  const fns = [];
  for (let i = 0; i < lines.length; i += 1) {
    const match = lines[i].match(
      /^\s*(pub(\([^)]*\))?\s+)?fn\s+([A-Za-z0-9_]+)\s*\(([^)]*)\)/,
    );
    if (!match) continue;
    const end = scanRustFunctionEnd(lines, i);
    fns.push({
      name: match[3],
      start: i + 1,
      end: end + 1,
      loc: end - i + 1,
      paramCount: countRustParameters(match[4]),
    });
  }
  return fns;
}
