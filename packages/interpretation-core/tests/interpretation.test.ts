import test from "node:test";
import assert from "node:assert/strict";

import {
  extractTransitions,
  interpretGrid,
  type GridSnapshot
} from "../src/index";

test("extractTransitions detects activations and deactivations", () => {
  const previous: GridSnapshot = { width: 3, height: 2, cells: [false, true, false, true, false, false] };
  const next: GridSnapshot = { width: 3, height: 2, cells: [true, true, false, false, false, true] };

  const transitions = extractTransitions(previous, next);
  assert.equal(transitions.length, 3);
  assert.deepEqual(transitions, [
    { x: 0, y: 0, kind: "activate" },
    { x: 0, y: 1, kind: "deactivate" },
    { x: 2, y: 1, kind: "activate" }
  ]);
});

test("interpretGrid combines event and scan-row state intents with degrees", () => {
  const previous: GridSnapshot = { width: 3, height: 3, cells: [false, false, false, false, false, false, false, false, false] };
  const next: GridSnapshot = { width: 3, height: 3, cells: [false, false, false, true, true, false, false, false, false] };

  const intents = interpretGrid(previous, next, 1, {
    id: "combined",
    event: { enabled: true },
    state: { enabled: true, tick: { mode: "scan_row_active" } },
    x: { mode: "scale_step", step: 2 },
    y: { mode: "scale_step", step: 3 }
  });

  assert.equal(intents.length, 5);
  const scanned = intents.filter((i) => i.kind === "scanned");
  assert.equal(scanned.length, 2);
  assert.equal(intents.filter((i) => i.kind === "scanned_empty").length, 1);
  assert.ok(intents.every((i) => i.degree >= 0));
});

test("scan-row emits scanned_empty for dead cells", () => {
  const previous: GridSnapshot = { width: 3, height: 3, cells: [false, false, false, false, false, false, false, false, false] };
  const next: GridSnapshot = { width: 3, height: 3, cells: [false, false, false, true, false, true, false, false, false] };
  const intents = interpretGrid(previous, next, 1, {
    id: "scan-empty",
    event: { enabled: false },
    state: { enabled: true, tick: { mode: "scan_row_active" } },
    x: { mode: "ignore" },
    y: { mode: "ignore" }
  });
  assert.equal(intents.length, 3);
  assert.equal(intents.filter((i) => i.kind === "scanned").length, 2);
  assert.equal(intents.filter((i) => i.kind === "scanned_empty").length, 1);
});

test("scan-column emits scanned and scanned_empty for all rows", () => {
  const grid: GridSnapshot = { width: 3, height: 3, cells: [false, true, false, false, false, false, false, true, false] };
  const intents = interpretGrid(grid, grid, 1, {
    id: "scan-col",
    event: { enabled: false },
    state: { enabled: true, tick: { mode: "scan_column_active" } },
    x: { mode: "ignore" },
    y: { mode: "ignore" }
  });
  assert.equal(intents.length, 3);
  assert.equal(intents.filter((i) => i.kind === "scanned").length, 2);
  assert.equal(intents.filter((i) => i.kind === "scanned_empty").length, 1);
});

test("whole-grid-active emits only active scanned cells", () => {
  const grid: GridSnapshot = { width: 2, height: 2, cells: [true, false, true, false] };
  const intents = interpretGrid(grid, grid, 0, {
    id: "all-active",
    event: { enabled: false },
    state: { enabled: true, tick: { mode: "whole_grid_active" } },
    x: { mode: "ignore" },
    y: { mode: "ignore" }
  });
  assert.equal(intents.length, 2);
  assert.equal(intents.every((i) => i.kind === "scanned"), true);
});

test("sectioned row scan sweeps horizontal lanes", () => {
  const grid: GridSnapshot = { width: 8, height: 8, cells: Array.from({ length: 64 }, (_, i) => i === 33 || i === 49 || i === 57) };
  const firstLane = interpretGrid(grid, grid, 1, { id: "sections", event: { enabled: false }, state: { enabled: true, tick: { mode: "scan_row_active", sections: 2 } }, x: { mode: "ignore" }, y: { mode: "ignore" } });
  const secondLane = interpretGrid(grid, grid, 9, { id: "sections", event: { enabled: false }, state: { enabled: true, tick: { mode: "scan_row_active", sections: 2 } }, x: { mode: "ignore" }, y: { mode: "ignore" } });

  assert.deepEqual(firstLane.map((i) => [i.x, i.y]), [[0, 1], [1, 1], [2, 1], [3, 1]]);
  assert.deepEqual(secondLane.map((i) => [i.x, i.y]), [[4, 1], [5, 1], [6, 1], [7, 1]]);
});
