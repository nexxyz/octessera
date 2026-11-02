export type GridTile = { x: number; y: number };

export type GridDomain = {
  width: number;
  height: number;
  toLogicalTile(inputTile: GridTile): GridTile;
  toDisplayTile(logicalTile: GridTile): GridTile;
  indexOf(logicalTile: GridTile): number;
  tileOf(index: number): GridTile;
  toLogicalIndex(inputTile: GridTile): number;
  toDisplayIndex(logicalTile: GridTile): number;
  displayTileOf(index: number): GridTile;
  get(tiles: boolean[], logicalTile: GridTile): boolean;
  set(tiles: boolean[], logicalTile: GridTile, value: boolean): boolean[];
  toggle(tiles: boolean[], logicalTile: GridTile): boolean[];
};

export function createGridDomain(width: number, height: number): GridDomain {
  const clampX = (x: number) => clampInt(x, 0, width - 1);
  const clampY = (y: number) => clampInt(y, 0, height - 1);

  const toLogicalTile = (inputTile: GridTile): GridTile => ({
    x: clampX(inputTile.x),
    y: clampY((height - 1) - inputTile.y)
  });

  const toDisplayTile = (logicalTile: GridTile): GridTile => ({
    x: clampX(logicalTile.x),
    y: clampY((height - 1) - logicalTile.y)
  });

  const indexOf = (logicalTile: GridTile): number => clampY(logicalTile.y) * width + clampX(logicalTile.x);

  const tileOf = (index: number): GridTile => {
    const i = clampInt(index, 0, width * height - 1);
    return { x: i % width, y: Math.floor(i / width) };
  };

  const toLogicalIndex = (inputTile: GridTile): number => indexOf(toLogicalTile(inputTile));

  const toDisplayIndex = (logicalTile: GridTile): number => {
    const display = toDisplayTile(logicalTile);
    return display.y * width + display.x;
  };

  const displayTileOf = (index: number): GridTile => toDisplayTile(tileOf(index));

  const get = (tiles: boolean[], logicalTile: GridTile): boolean => !!tiles[indexOf(logicalTile)];

  const set = (tiles: boolean[], logicalTile: GridTile, value: boolean): boolean[] => {
    const idx = indexOf(logicalTile);
    const next = tiles.slice();
    next[idx] = value;
    return next;
  };

  const toggle = (tiles: boolean[], logicalTile: GridTile): boolean[] => {
    const idx = indexOf(logicalTile);
    const next = tiles.slice();
    next[idx] = !next[idx];
    return next;
  };

  return { width, height, toLogicalTile, toDisplayTile, indexOf, tileOf, toLogicalIndex, toDisplayIndex, displayTileOf, get, set, toggle };
}

function clampInt(value: number, min: number, max: number): number {
  const n = Math.floor(value);
  return Math.max(min, Math.min(max, n));
}
