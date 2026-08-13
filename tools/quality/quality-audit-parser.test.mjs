import assert from "node:assert/strict";
import test from "node:test";

import {
  scanApproximateRustFunctions,
  scanJavaScriptFunctions,
  scanJavaScriptFunctionsWithFallback,
} from "./quality-audit-parser.mjs";

const findFunction = (functions, name) => {
  const result = functions.find((entry) => entry.name === name);
  assert.ok(result, `expected function ${name}`);
  return result;
};

test("scans multiline async functions and excludes nested function bodies from complexity", () => {
  const source = `
    export async function complex(
      value: string,
      options: { enabled: boolean } = { enabled: true },
      ...rest: string[]
    ) {
      // A comment with braces: { }
      const template = \`value { ${"${value}"} }\`;
      if ((options.enabled && value) ?? false) {
        for (const item of rest) {
          for (const key in options) {
            for (let index = 0; index < 1; index += 1) {
              void index;
            }
            while (item) {
              do { break; } while (value);
            }
          }
        }
        try { return template; } catch (error) { return error; }
        switch (value) {
          case "one": return template;
          default: return template;
        }
      }
      const inner = () => {
        if (value || options.enabled) return true;
        return false;
      };
      function declaredNested() { if (value) return value; return value; }
      return value ? template : rest[0];
    }
  `;
  const functions = scanJavaScriptFunctions(source, ".ts");
  const complex = findFunction(functions, "complex");
  const inner = findFunction(functions, "inner");
  const declaredNested = findFunction(functions, "declaredNested");

  assert.equal(complex.start, 2);
  assert.equal(complex.loc, 31);
  assert.equal(complex.paramCount, 3);
  assert.equal(complex.complexity, 12);
  assert.equal(inner.complexity, 3);
  assert.equal(declaredNested.complexity, 2);
});

test("counts arrows, expressions, object methods, class methods, accessors, and private methods", () => {
  const source = `
    const assigned = function () { return true; };
    const arrow = async (first: string, second = 2, ...rest: number[]) => first + second + rest.length;
    const destructured = ({ first, second: { nested } } = { first: "", second: { nested: 0 } }, ...rest) => first + nested + rest.length;
    const object = {
      run(value: string) { return value; },
      get value() { return 1; },
      set value(next: number) { void next; }
    };
    class Synth {
      run() { return true; }
      get level() { return 1; }
      set level(next: number) { void next; }
      #secret() { return false; }
    }
  `;
  const functions = scanJavaScriptFunctions(source, ".ts");

  assert.deepEqual(
    functions.map(({ name }) => name),
    [
      "assigned",
      "arrow",
      "destructured",
      "run",
      "get value",
      "set value",
      "run",
      "get level",
      "set level",
      "#secret",
    ],
  );
  assert.equal(findFunction(functions, "arrow").paramCount, 3);
  assert.equal(findFunction(functions, "destructured").paramCount, 2);
  assert.equal(functions.filter(({ name }) => name === "run").length, 2);
});

test("parses TSX and measures loc from AST locations", () => {
  const source = `
    type Props = { enabled: boolean };
    const View = (props: Props) => (
      <section className={props.enabled ? "on" : "off"}>
        {props.enabled && <span>{\`ready {now}\`}</span>}
      </section>
    );
  `;
  const functions = scanJavaScriptFunctions(source, ".tsx");
  const view = findFunction(functions, "View");

  assert.equal(view.start, 3);
  assert.equal(view.end, 7);
  assert.equal(view.loc, 5);
  assert.equal(view.complexity, 3);
  assert.equal(view.paramCount, 1);
});

test("parses modern MJS function declarations with the standard Babel grammar", () => {
  const functions = scanJavaScriptFunctions(
    "export function mjsFunction(value) { return value?.enabled ?? false; }",
    ".mjs",
  );

  assert.deepEqual(functions, [
    {
      name: "mjsFunction",
      start: 1,
      end: 1,
      loc: 1,
      complexity: 2,
      paramCount: 1,
    },
  ]);
});

test("parses ordinary JS functions and measures executable complexity", () => {
  const functions = scanJavaScriptFunctions(
    "function plain(value) { if (value && value.ready) return value; return null; }",
    ".js",
  );

  assert.deepEqual(functions, [
    {
      name: "plain",
      start: 1,
      end: 1,
      loc: 1,
      complexity: 3,
      paramCount: 1,
    },
  ]);
});

test("warns and uses the approximate JavaScript scanner after a Babel parse failure", () => {
  const warnings = [];
  const functions = scanJavaScriptFunctionsWithFallback(
    "function broken() { return ???; }",
    ".mjs",
    (warning) => warnings.push(warning),
  );

  assert.equal(functions.length, 1);
  assert.equal(functions[0].name, "broken");
  assert.match(warnings[0], /Babel parse failed/);
  assert.match(warnings[0], /approximate JavaScript regex scanner/);
});

test("keeps Rust measurements on the explicit approximate scanner", () => {
  const functions = scanApproximateRustFunctions(`
    pub fn render(value: usize, other: usize) {
      if value > other { return; }
    }
  `);

  assert.deepEqual(functions, [
    {
      name: "render",
      start: 2,
      end: 4,
      loc: 3,
      complexity: 2,
      paramCount: 2,
    },
  ]);
});
