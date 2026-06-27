import { useState } from "react";
import { AUX_ENCODER_COUNT, type DeviceInput } from "@cellsymphony/device-contracts";
import { createSimulatorRuntime } from "../runtime/simulatorRuntime";
import { ControlsPanel } from "./ControlsPanel";
import { GridMatrix } from "./GridMatrix";
import { useGridInteraction } from "./appGridInteraction";
import { useDialDragBindings, useKeyboardBindings, useRuntimeBindings } from "./appHooks";

const runtime = createSimulatorRuntime();
type EncoderId = "main" | `aux${number}`;
const INITIAL_DIAL_PHASE = Object.fromEntries([
  ["main", 0],
  ...Array.from({ length: AUX_ENCODER_COUNT }, (_, index) => [`aux${index + 1}`, 0] as const)
]);

export function App() {
  const [snapshot, setSnapshot] = useState(() => runtime.getSnapshot());
  const [dialDrag, setDialDrag] = useState<{ id: EncoderId; y: number; acc: number } | null>(null);
  const [dialPhase, setDialPhase] = useState<Record<string, number>>(INITIAL_DIAL_PHASE);
  const frame = snapshot.frame;

  function dispatch(input: DeviceInput) {
    if (input.type === "encoder_turn") {
      bumpDialPhase(input.id, input.delta);
    }
    runtime.dispatch(input);
  }

  function bumpDialPhase(id: EncoderId | undefined, delta: number) {
    const step = Math.sign(delta);
    if (step === 0) return;
    const key = id ?? "main";
    setDialPhase((prev) => ({ ...prev, [key]: ((prev[key] ?? 0) + step + 8) % 8 }));
  }

  function turnWithAcceleration(id: EncoderId, delta: -1 | 1, magnitude: number) {
    const turns = magnitude >= 90 ? 4 : magnitude >= 40 ? 2 : 1;
    for (let i = 0; i < turns; i += 1) dispatch({ type: "encoder_turn", delta, id });
  }

  function setModifier(kind: "shift" | "fn", active: boolean) {
    runtime.dispatchAction({ type: kind, active });
  }
  const { handleMouseUp, handleCellMouseDown, handleCellDrag } = useGridInteraction(frame, dispatch);

  useRuntimeBindings(runtime, setSnapshot);
  useKeyboardBindings(runtime, bumpDialPhase);
  useDialDragBindings(dialDrag, setDialDrag, turnWithAcceleration);

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
