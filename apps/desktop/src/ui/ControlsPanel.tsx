import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { SimulatorSnapshot } from "../runtime/types";
import { OledDisplay } from "./OledDisplay";

type EncoderId = "main" | "aux1" | "aux2" | "aux3" | "aux4";

const ENCODERS = [
  { id: "main", label: "SW1", active: true },
  { id: "aux1", label: "SW2", active: false },
  { id: "aux2", label: "SW3", active: false },
  { id: "aux3", label: "SW4", active: false },
  { id: "aux4", label: "SW5", active: false }
] as const;

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
        <h3>{ENCODERS[0].label}</h3>
        <Dial id="main" phase={dialPhase.main ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Menu Control</small>
      </article>

      <OledDisplay frame={frame} displayBrightness={snapshot.displayBrightness} />

      <article className="encoder-card sw2">
        <h3>{ENCODERS[1].label}</h3>
        <Dial id="aux1" phase={dialPhase.aux1 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Aux Control</small>
      </article>
      <article className="encoder-card sw3">
        <h3>{ENCODERS[2].label}</h3>
        <Dial id="aux2" phase={dialPhase.aux2 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Aux Control</small>
      </article>
      <section className="button-stack stack-a">
        {NEOKEY_BUTTONS.slice(0, 2).map((button) => (
          <NeoKey key={button.key} button={button} dispatch={dispatch} setModifier={setModifier} snapshot={snapshot} />
        ))}
      </section>

      <article className="encoder-card sw4">
        <h3>{ENCODERS[3].label}</h3>
        <Dial id="aux3" phase={dialPhase.aux3 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Aux Control</small>
      </article>
      <article className="encoder-card sw5">
        <h3>{ENCODERS[4].label}</h3>
        <Dial id="aux4" phase={dialPhase.aux4 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
        <small>Aux Control</small>
      </article>
      <section className="button-stack stack-b">
        {NEOKEY_BUTTONS.slice(2).map((button) => (
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
      className={`neokey-${button.key} ${snapshot.neoKeyLeds[button.key]}`}
      style={{ opacity: Math.max(0.25, snapshot.buttonBrightness / 100) }}
    >
      {button.label}
    </button>
  );
}
