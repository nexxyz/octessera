import { useRef, useState } from "react";
import { GRID_DOMAIN, type DeviceInput, type RuntimeSnapshot } from "@octessera/device-contracts";

export function useGridInteraction(frame: RuntimeSnapshot, dispatch: (input: DeviceInput) => void) {
  const [paintMode, setPaintMode] = useState<boolean | null>(null);
  const [painted, setPainted] = useState<Set<string>>(new Set());
  const lastPressedCell = useRef<{ x: number; y: number } | null>(null);

  function cellAlive(index: number): boolean {
    return (frame.leds.rgb[index * 3 + 1] ?? 0) > 100;
  }

  function logicalCellFromDisplay(x: number, y: number) {
    return GRID_DOMAIN.toLogicalCell({ x, y });
  }

  function applyPaint(x: number, y: number, desired: boolean) {
    const key = `${x}-${y}`;
    if (painted.has(key)) return;
    const index = GRID_DOMAIN.toDisplayIndex(GRID_DOMAIN.toLogicalCell({ x, y }));
    if (cellAlive(index) !== desired) {
      const world = logicalCellFromDisplay(x, y);
      dispatch({ type: "grid_press", x: world.x, y: world.y });
    }
    setPainted((prev) => new Set(prev).add(key));
  }

  function pressMomentaryCell(x: number, y: number) {
    const world = logicalCellFromDisplay(x, y);
    const previous = lastPressedCell.current;
    const sameCell = previous?.x === world.x && previous.y === world.y;
    if (sameCell) return;
    if (previous) dispatch({ type: "grid_release", x: previous.x, y: previous.y });
    dispatch({ type: "grid_press", x: world.x, y: world.y });
    lastPressedCell.current = world;
  }

  function endPaint() {
    setPaintMode(null);
    setPainted(new Set());
  }

  function handleMouseUp() {
    if (lastPressedCell.current) {
      dispatch({ type: "grid_release", x: lastPressedCell.current.x, y: lastPressedCell.current.y });
      lastPressedCell.current = null;
    }
    endPaint();
  }

  function handleCellMouseDown(index: number, x: number, y: number) {
    if (frame.gridInteraction === "momentary") {
      setPaintMode(null);
      setPainted(new Set());
      pressMomentaryCell(x, y);
      return;
    }
    const desired = !cellAlive(index);
    setPaintMode(desired);
    setPainted(new Set());
    lastPressedCell.current = logicalCellFromDisplay(x, y);
    applyPaint(x, y, desired);
  }

  function handleCellDrag(x: number, y: number) {
    if (frame.gridInteraction === "momentary") {
      pressMomentaryCell(x, y);
      return;
    }
    if (paintMode === null) return;
    applyPaint(x, y, paintMode);
  }

  return { handleMouseUp, handleCellMouseDown, handleCellDrag };
}
