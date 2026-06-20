import { useRef, useState } from "react";
import { AUX_ENCODER_COUNT, GRID_DOMAIN, type DeviceInput } from "@cellsymphony/device-contracts";
import { createSimulatorRuntime } from "../runtime/simulatorRuntime";
import { ControlsPanel } from "./ControlsPanel";
import { GridMatrix } from "./GridMatrix";
import { useAudioConfigSync, useDialDragBindings, useKeyboardBindings, useRuntimeBindings } from "./appHooks";

const runtime = createSimulatorRuntime();
type EncoderId = "main" | `aux${number}`;
const INITIAL_DIAL_PHASE = Object.fromEntries([
  ["main", 0],
  ...Array.from({ length: AUX_ENCODER_COUNT }, (_, index) => [`aux${index + 1}`, 0] as const)
]);

export function App() {
  const [snapshot, setSnapshot] = useState(() => runtime.getSnapshot());
  const [paintMode, setPaintMode] = useState<boolean | null>(null);
  const [painted, setPainted] = useState<Set<string>>(new Set());
  const [dialDrag, setDialDrag] = useState<{ id: EncoderId; y: number; acc: number } | null>(null);
  const [dialPhase, setDialPhase] = useState<Record<string, number>>(INITIAL_DIAL_PHASE);
  const lastPressedCell = useRef<{ x: number; y: number } | null>(null);
  const frame = snapshot.frame;
  function dispatch(input: DeviceInput) {
    if (input.type === "encoder_turn") {
      bumpDialPhase(input.id, input.delta);
    }
    runtime.dispatch(input);
  }

  function bumpDialPhase(id: EncoderId | undefined, delta: -1 | 1) {
    const key = id ?? "main";
    setDialPhase((prev) => ({ ...prev, [key]: ((prev[key] ?? 0) + delta + 8) % 8 }));
  }

  function turnWithAcceleration(id: EncoderId, delta: -1 | 1, magnitude: number) {
    const turns = magnitude >= 90 ? 4 : magnitude >= 40 ? 2 : 1;
    for (let i = 0; i < turns; i += 1) dispatch({ type: "encoder_turn", delta, id });
  }

  function cellAlive(index: number): boolean {
    const c = frame.leds.cells[index];
    return c.g > 100;
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

  function setModifier(kind: "shift" | "fn", active: boolean) {
    runtime.dispatchAction({ type: kind, active });
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

  useRuntimeBindings(runtime, setSnapshot);
  useKeyboardBindings(runtime, bumpDialPhase);
  useDialDragBindings(dialDrag, setDialDrag, turnWithAcceleration);
  useAudioConfigSync(snapshot);

  return (
    <main className="app-shell" onMouseUp={handleMouseUp} onMouseLeave={handleMouseUp}>
      <header className="bar">Cell Symphony Hardware Simulator</header>
      <section className="panel-layout">
        <ControlsPanel
          dialPhase={dialPhase}
          dispatch={dispatch}
          frame={frame}
          setDialDrag={setDialDrag}
          setModifier={setModifier}
          snapshot={snapshot}
          turnWithAcceleration={turnWithAcceleration}
        />

        <GridMatrix frame={frame} onCellDrag={handleCellDrag} onCellMouseDown={handleCellMouseDown} />
      </section>

      <footer className="bar footer">Left/Right/Up/Down or Wheel: SW1 turn • Enter: SW1 press • Backspace: Back • Space: Play/Pause • Shift+Space: Stop</footer>
    </main>
  );
}
