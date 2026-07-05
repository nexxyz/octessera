import type { DeviceInput } from "@cellsymphony/device-contracts";
import { AUX_ENCODER_COUNT } from "@cellsymphony/device-contracts";
import type { CSSProperties } from "react";
import type { SimulatorSnapshot } from "../runtime/types";
import { OledDisplay } from "./OledDisplay";

type EncoderId = "main" | `aux${number}`;

const AUX_ENCODERS = Array.from({ length: AUX_ENCODER_COUNT }, (_, index) => ({
  id: `aux${index + 1}` as const,
  label: `SW${index + 2}`,
  className: `aux-${index + 1}`
}));

const NEOKEY_BUTTONS = [
  { input: { type: "button_a" } as DeviceInput, label: "Back", key: "back" as const },
  { input: { type: "button_s" } as DeviceInput, label: "Space ▶/⏸", key: "space" as const },
  { input: { type: "button_shift" } as DeviceInput, label: "Shift", key: "shift" as const },
  { input: { type: "button_fn" } as DeviceInput, label: "Fn (Ctrl)", key: "fn" as const }
];

export function ControlsPanel({
  dialPhase,
  dispatch,
  frame,
  setDialDrag,
  setModifier,
  snapshot,
  turnWithAcceleration
}: {
  dialPhase: Record<string, number>;
  dispatch: (input: DeviceInput) => void;
  frame: SimulatorSnapshot["frame"];
  setDialDrag: (state: { id: EncoderId; y: number; acc: number } | null) => void;
  setModifier: (kind: "shift" | "fn", active: boolean) => void;
  snapshot: SimulatorSnapshot;
  turnWithAcceleration: (id: EncoderId, delta: -1 | 1, magnitude: number) => void;
}) {
  return (
    <section className="control-grid">
      <article className="encoder-card sw1">
        <h3>SW1</h3>
        <Dial id="main" phase={dialPhase.main ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Menu Control</small>
      </article>

      <OledDisplay audioLoad={snapshot.audioLoad} frame={frame} displayBrightness={snapshot.displayBrightness} />

      <section className="aux-triangle" aria-label="Aux encoders">
        {AUX_ENCODERS.map((encoder) => (
          <article key={encoder.id} className={`encoder-card ${encoder.className}`}>
            <h3>{encoder.label}</h3>
            <Dial id={encoder.id} phase={dialPhase[encoder.id] ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
            <small>Aux Control</small>
          </article>
        ))}
      </section>
      <section className="button-stack button-block">
        {NEOKEY_BUTTONS.map((button) => (
          <NeoKey key={button.key} button={button} dispatch={dispatch} setModifier={setModifier} snapshot={snapshot} />
        ))}
      </section>
    </section>
  );
}

function Dial({
  id,
  phase,
  dispatch,
  setDialDrag,
  turnWithAcceleration
}: {
  id: EncoderId;
  phase: number;
  dispatch: (input: DeviceInput) => void;
  setDialDrag: (state: { id: EncoderId; y: number; acc: number } | null) => void;
  turnWithAcceleration: (id: EncoderId, delta: -1 | 1, magnitude: number) => void;
}) {
  return (
    <div
      className="encoder-dial"
      onMouseDown={(event) => {
        event.preventDefault();
        setDialDrag({ id, y: event.clientY, acc: 0 });
      }}
      onWheel={(event) => {
        event.preventDefault();
        turnWithAcceleration(id, event.deltaY > 0 ? 1 : -1, Math.abs(event.deltaY));
      }}
    >
      <div className="encoder-ring" aria-hidden="true">
        {Array.from({ length: 8 }, (_, i) => (
          <span
            key={`${id}-tick-${i}`}
            className={`encoder-tick ${i === phase ? "active" : ""}`}
            style={{ transform: `translate(-50%, -50%) rotate(${i * 45}deg) translateY(-45px)` }}
          />
        ))}
      </div>
      <button type="button" className="encoder-center" onClick={() => dispatch({ type: "encoder_press", id })}>
        Push
      </button>
    </div>
  );
}

function NeoKey({
  button,
  dispatch,
  setModifier,
  snapshot
}: {
  button: (typeof NEOKEY_BUTTONS)[number];
  dispatch: (input: DeviceInput) => void;
  setModifier: (kind: "shift" | "fn", active: boolean) => void;
  snapshot: SimulatorSnapshot;
}) {
  return (
    <button
      type="button"
      onClick={(event) => {
        if (button.key === "shift" || button.key === "fn") {
          event.preventDefault();
          return;
        }
        dispatch(button.input);
      }}
      onMouseDown={() => {
        if (button.key === "shift") setModifier("shift", true);
        if (button.key === "fn") setModifier("fn", true);
      }}
      onMouseUp={() => {
        if (button.key === "shift") setModifier("shift", false);
        if (button.key === "fn") setModifier("fn", false);
      }}
      onMouseLeave={() => {
        if (button.key === "shift") setModifier("shift", false);
        if (button.key === "fn") setModifier("fn", false);
      }}
      className={`neokey-${button.key}`}
      style={neoKeyStyle(snapshot.neoKeyLeds[button.key])}
    >
      {button.label}
    </button>
  );
}

function neoKeyStyle(rgb: [number, number, number]): CSSProperties {
  const [r, g, b] = rgb;
  const color = `rgb(${r}, ${g}, ${b})`;
  const lit = Math.max(r, g, b) > 0;
  return {
    backgroundColor: color,
    borderColor: lit ? `rgb(${Math.min(255, r + 64)}, ${Math.min(255, g + 64)}, ${Math.min(255, b + 64)})` : undefined,
    boxShadow: lit ? `0 0 14px rgba(${r}, ${g}, ${b}, 0.55)` : "none",
  };
}
