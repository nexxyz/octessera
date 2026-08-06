import { useRef, useState } from 'react';
import {
  GRID_DOMAIN,
  type DeviceInput,
  type RuntimeSnapshot,
} from '@octessera/device-contracts';

type GridCell = { x: number; y: number };

export function gridLogicalCellFromDisplay(x: number, y: number): GridCell {
  return GRID_DOMAIN.toLogicalCell({ x, y });
}

export function gridDisplayIndexFromDisplay(x: number, y: number): number {
  return GRID_DOMAIN.toDisplayIndex(gridLogicalCellFromDisplay(x, y));
}

export function paintCellTransition({
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
  const world = gridLogicalCellFromDisplay(x, y);
  return {
    key,
    painted: new Set(painted).add(key),
    input:
      active === desired
        ? null
        : { type: 'grid_press', x: world.x, y: world.y },
  };
}

export function momentaryCellTransition(
  previous: GridCell | null,
  x: number,
  y: number,
): { current: GridCell; inputs: DeviceInput[] } {
  const current = gridLogicalCellFromDisplay(x, y);
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
  const lastPressedCell = useRef<{ x: number; y: number } | null>(null);

  function cellAlive(index: number): boolean {
    return frame.leds.active[index] ?? false;
  }

  function logicalCellFromDisplay(x: number, y: number) {
    return gridLogicalCellFromDisplay(x, y);
  }

  function applyPaint(x: number, y: number, desired: boolean) {
    const key = `${x}-${y}`;
    if (paintedRef.current.has(key)) return;
    const transition = paintCellTransition({
      painted: paintedRef.current,
      active: cellAlive(
        GRID_DOMAIN.toDisplayIndex(logicalCellFromDisplay(x, y)),
      ),
      desired,
      x,
      y,
    });
    if (transition.input) dispatch(transition.input);
    paintedRef.current = transition.painted;
    setPainted(transition.painted);
  }

  function pressMomentaryCell(x: number, y: number) {
    const previous = lastPressedCell.current;
    const transition = momentaryCellTransition(previous, x, y);
    transition.inputs.forEach(dispatch);
    lastPressedCell.current = transition.current;
  }

  function endPaint() {
    setPaintMode(null);
    paintedRef.current = new Set();
    setPainted(new Set());
  }

  function handleMouseUp() {
    if (lastPressedCell.current) {
      dispatch({
        type: 'grid_release',
        x: lastPressedCell.current.x,
        y: lastPressedCell.current.y,
      });
      lastPressedCell.current = null;
    }
    endPaint();
  }

  function handleCellMouseDown(index: number, x: number, y: number) {
    if (frame.gridInteraction === 'momentary') {
      setPaintMode(null);
      paintedRef.current = new Set();
      setPainted(new Set());
      pressMomentaryCell(x, y);
      return;
    }
    const desired = !cellAlive(index);
    setPaintMode(desired);
    paintedRef.current = new Set();
    setPainted(new Set());
    lastPressedCell.current = logicalCellFromDisplay(x, y);
    applyPaint(x, y, desired);
  }

  function handleCellDrag(x: number, y: number) {
    if (frame.gridInteraction === 'momentary') {
      pressMomentaryCell(x, y);
      return;
    }
    if (paintMode === null) return;
    applyPaint(x, y, paintMode);
  }

  return { handleMouseUp, handleCellMouseDown, handleCellDrag };
}
