import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH } from "../src/index";

type GridProjectionFixtureCoordinate = { x: number; y: number; index: number };
type GridProjectionFixture = {
  version: number;
  width: number;
  height: number;
  cells: Array<{
    logical: GridProjectionFixtureCoordinate;
    display: GridProjectionFixtureCoordinate;
    neotrellis: { device_index: number; address: string; key: number; seesaw_key: number };
  }>;
};

const loadGridProjectionFixture = (): GridProjectionFixture =>
  JSON.parse(
    readFileSync(new URL("../../../resources/grid-projection-v1.json", import.meta.url), "utf8"),
  ) as GridProjectionFixture;

test("grid domain mirrors every native projection fixture cell", () => {
  const fixture = loadGridProjectionFixture();
  assert.equal(fixture.version, 1);
  assert.equal(fixture.width, GRID_WIDTH);
  assert.equal(fixture.height, GRID_HEIGHT);
  assert.equal(fixture.cells.length, GRID_WIDTH * GRID_HEIGHT);

  const logicalIndices = new Set<number>();
  const displayIndices = new Set<number>();
  const trellisCells = new Set<string>();
  const addresses = ["0x2e", "0x2f", "0x30", "0x31"];
  for (const cell of fixture.cells) {
    assertGridProjectionCell(cell, addresses);
    assert.equal(logicalIndices.has(cell.logical.index), false);
    assert.equal(displayIndices.has(cell.display.index), false);
    assert.equal(
      trellisCells.has(`${cell.neotrellis.device_index}:${cell.neotrellis.key}`),
      false,
    );
    logicalIndices.add(cell.logical.index);
    displayIndices.add(cell.display.index);
    trellisCells.add(`${cell.neotrellis.device_index}:${cell.neotrellis.key}`);
  }
  assert.equal(logicalIndices.size, 64);
  assert.equal(displayIndices.size, 64);
  assert.equal(trellisCells.size, 64);
});

function assertGridProjectionCell(
  cell: GridProjectionFixture["cells"][number],
  addresses: readonly string[],
): void {
  const { logical, display, neotrellis } = cell;
  assert.ok(logical.x >= 0 && logical.x < GRID_WIDTH);
  assert.ok(logical.y >= 0 && logical.y < GRID_HEIGHT);
  assert.ok(logical.index >= 0 && logical.index < GRID_WIDTH * GRID_HEIGHT);
  assert.ok(display.x >= 0 && display.x < GRID_WIDTH);
  assert.ok(display.y >= 0 && display.y < GRID_HEIGHT);
  assert.ok(display.index >= 0 && display.index < GRID_WIDTH * GRID_HEIGHT);
  assert.equal(GRID_DOMAIN.indexOf(logical), logical.index);
  assert.deepEqual(GRID_DOMAIN.cellOf(logical.index), { x: logical.x, y: logical.y });
  assert.deepEqual(GRID_DOMAIN.toDisplayCell(logical), { x: display.x, y: display.y });
  assert.equal(GRID_DOMAIN.toDisplayIndex(logical), display.index);
  assert.deepEqual(GRID_DOMAIN.toLogicalCell(display), { x: logical.x, y: logical.y });
  assert.equal(GRID_DOMAIN.toLogicalIndex(display), logical.index);
  assert.deepEqual(GRID_DOMAIN.displayCellOf(logical.index), { x: display.x, y: display.y });
  assert.equal(neotrellis.address, addresses[neotrellis.device_index]);
  assert.ok(neotrellis.device_index >= 0 && neotrellis.device_index < 4);
  assert.ok(neotrellis.key >= 0 && neotrellis.key < 16);
  assert.ok(neotrellis.seesaw_key >= 0 && neotrellis.seesaw_key < 32);
}
