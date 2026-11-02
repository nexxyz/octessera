import { useRef, useState } from 'react';
import {
  GRID_DOMAIN,
  type DeviceInput,
  type RuntimeSnapshot,
} from '@octessera/device-contracts';

type GridTile = { x: number; y: number };

export function gridLogicalTileFromDisplay(x: number, y: number): GridTile {
  return GRID_DOMAIN.toLogicalTile({ x, y });
}

export function gridDisplayIndexFromDisplay(x: number, y: number): number {
  return GRID_DOMAIN.toDisplayIndex(gridLogicalTileFromDisplay(x, y));
}

export function paintTileTransition({
  painted,
  active,
  desired,
  x,
  y,
}: {
  painted: ReadonlySet<string>;
  active: boolean;
  desired: boolean;
  x: number;
  y: number;
}): { key: string; painted: Set<string>; input: DeviceInput | null } {
  const key = `${x}-${y}`;
  if (painted.has(key)) return { key, painted: new Set(painted), input: null };
  const world = gridLogicalTileFromDisplay(x, y);
  return {
    key,
    painted: new Set(painted).add(key),
    input:
      active === desired
        ? null
        : { type: 'grid_press', x: world.x, y: world.y },
  };
}

export function momentaryTileTransition(
  previous: GridTile | null,
  x: number,
  y: number,
): { current: GridTile; inputs: DeviceInput[] } {
  const current = gridLogicalTileFromDisplay(x, y);
  if (previous?.x === current.x && previous.y === current.y)
    return { current: previous, inputs: [] };
  return {
    current,
    inputs: [
      ...(previous
        ? [
            {
              type: 'grid_release',
              x: previous.x,
              y: previous.y,
            } as DeviceInput,
          ]
        : []),
      { type: 'grid_press', x: current.x, y: current.y },
    ],
  };
}

export function useGridInteraction(
  frame: RuntimeSnapshot,
  dispatch: (input: DeviceInput) => void,
) {
  const [paintMode, setPaintMode] = useState<boolean | null>(null);
  const [, setPainted] = useState<Set<string>>(new Set());
  const paintedRef = useRef(new Set<string>());
  const lastPressedTile = useRef<{ x: number; y: number } | null>(null);

  function tileAlive(index: number): boolean {
    return frame.leds.active[index] ?? false;
  }

  function logicalTileFromDisplay(x: number, y: number) {
    return gridLogicalTileFromDisplay(x, y);
  }

  function applyPaint(x: number, y: number, desired: boolean) {
    const key = `${x}-${y}`;
    if (paintedRef.current.has(key)) return;
    const transition = paintTileTransition({
      painted: paintedRef.current,
      active: tileAlive(
        GRID_DOMAIN.toDisplayIndex(logicalTileFromDisplay(x, y)),
      ),
      desired,
      x,
      y,
    });
    if (transition.input) dispatch(transition.input);
    paintedRef.current = transition.painted;
    setPainted(transition.painted);
  }

  function pressMomentaryTile(x: number, y: number) {
    const previous = lastPressedTile.current;
    const transition = momentaryTileTransition(previous, x, y);
    transition.inputs.forEach(dispatch);
    lastPressedTile.current = transition.current;
  }

  function endPaint() {
    setPaintMode(null);
    paintedRef.current = new Set();
    setPainted(new Set());
  }

  function handleMouseUp() {
    if (lastPressedTile.current) {
      dispatch({
        type: 'grid_release',
        x: lastPressedTile.current.x,
        y: lastPressedTile.current.y,
      });
      lastPressedTile.current = null;
    }
    endPaint();
  }

  function handleTileMouseDown(index: number, x: number, y: number) {
    if (frame.gridInteraction === 'momentary') {
      setPaintMode(null);
      paintedRef.current = new Set();
      setPainted(new Set());
      pressMomentaryTile(x, y);
      return;
    }
    const desired = !tileAlive(index);
    setPaintMode(desired);
    paintedRef.current = new Set();
    setPainted(new Set());
    lastPressedTile.current = logicalTileFromDisplay(x, y);
    applyPaint(x, y, desired);
  }

  function handleTileDrag(x: number, y: number) {
    if (frame.gridInteraction === 'momentary') {
      pressMomentaryTile(x, y);
      return;
    }
    if (paintMode === null) return;
    applyPaint(x, y, paintMode);
  }

  return { handleMouseUp, handleTileMouseDown, handleTileDrag };
}
