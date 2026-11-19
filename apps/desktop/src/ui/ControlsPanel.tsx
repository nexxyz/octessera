import type { DeviceInput } from '@octessera/device-contracts';
import { AUX_ENCODER_COUNT } from '@octessera/device-contracts';
import type { CSSProperties } from 'react';
import type { SimulatorSnapshot } from '../runtime/types';
import { OledDisplay } from './OledDisplay';

type EncoderId = 'main' | `aux${number}`;
type ModifierKind = 'shift' | 'fn';

const WHEEL_PIXELS_PER_TURN = 100;

export function wheelTurnFromDelta(
  deltaY: number,
  deltaMode: number,
): {
  delta: -1 | 1;
  turns: number;
} | null {
  if (deltaY === 0) return null;
  const units =
    deltaMode === 0
      ? Math.abs(deltaY) / WHEEL_PIXELS_PER_TURN
      : Math.abs(deltaY);
  return { delta: deltaY > 0 ? 1 : -1, turns: Math.max(1, Math.round(units)) };
}

export function modifierForButton(key: string): ModifierKind | null {
  return key === 'shift' || key === 'fn' ? key : null;
}

export function buttonInputForClick(key: string): DeviceInput | null {
  if (key === 'back') return { type: 'button_a' };
  if (key === 'space') return { type: 'button_s' };
  return null;
}

const AUX_ENCODERS = Array.from({ length: AUX_ENCODER_COUNT }, (_, index) => ({
  id: `aux${index + 1}` as const,
  label: `SW${index + 2}`,
  className: `aux-${index + 1}`,
}));

const NEOKEY_BUTTONS = [
  {
    label: 'Back',
    key: 'back' as const,
  },
  {
    label: 'Space ▶/⏸',
    key: 'space' as const,
  },
  {
    input: { type: 'button_shift' } as DeviceInput,
    label: 'Shift',
    key: 'shift' as const,
  },
  {
    input: { type: 'button_fn' } as DeviceInput,
    label: 'Fn (Ctrl)',
    key: 'fn' as const,
  },
];

export function ControlsPanel({
  dialPhase,
  dispatch,
  frame,
  setDialDrag,
  setModifier,
  snapshot,
}: {
  dialPhase: Record<string, number>;
  dispatch: (input: DeviceInput) => void;
  frame: SimulatorSnapshot['frame'];
  setDialDrag: (
    state: { id: EncoderId; y: number; acc: number } | null,
  ) => void;
  setModifier: (kind: 'shift' | 'fn', active: boolean) => void;
  snapshot: SimulatorSnapshot;
}) {
  return (
    <section className="control-grid">
      <article className="encoder-card sw1">
        <h3>SW1</h3>
        <Dial
          id="main"
          phase={dialPhase.main ?? 0}
          dispatch={dispatch}
          setDialDrag={setDialDrag}
        />
        <small>Menu Control</small>
      </article>

      <OledDisplay
        frame={frame}
        displayBrightness={snapshot.displayBrightness}
      />

      <section className="aux-triangle" aria-label="Aux encoders">
        {AUX_ENCODERS.map((encoder) => (
          <article
            key={encoder.id}
            className={`encoder-card ${encoder.className}`}
          >
            <h3>{encoder.label}</h3>
            <Dial
              id={encoder.id}
              phase={dialPhase[encoder.id] ?? 0}
              dispatch={dispatch}
              setDialDrag={setDialDrag}
            />
            <small>Aux Control</small>
          </article>
        ))}
      </section>
      <section className="button-stack button-block">
        {NEOKEY_BUTTONS.map((button) => (
          <NeoKey
            key={button.key}
            button={button}
            dispatch={dispatch}
            setModifier={setModifier}
            snapshot={snapshot}
          />
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
}: {
  id: EncoderId;
  phase: number;
  dispatch: (input: DeviceInput) => void;
  setDialDrag: (
    state: { id: EncoderId; y: number; acc: number } | null,
  ) => void;
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
        const turn = wheelTurnFromDelta(event.deltaY, event.deltaMode);
        if (!turn) return;
        for (let i = 0; i < turn.turns; i += 1)
          dispatch({ type: 'encoder_turn', delta: turn.delta, id });
      }}
    >
      <div className="encoder-ring" aria-hidden="true">
        {Array.from({ length: 8 }, (_, i) => (
          <span
            key={`${id}-tick-${i}`}
            className={`encoder-tick ${i === phase ? 'active' : ''}`}
            style={{
              transform: `translate(-50%, -50%) rotate(${i * 45}deg) translateY(-45px)`,
            }}
          />
        ))}
      </div>
      <button
        type="button"
        className="encoder-center"
        onClick={() => dispatch({ type: 'encoder_press', id })}
      >
        Push
      </button>
    </div>
  );
}

function NeoKey({
  button,
  dispatch,
  setModifier,
  snapshot,
}: {
  button: (typeof NEOKEY_BUTTONS)[number];
  dispatch: (input: DeviceInput) => void;
  setModifier: (kind: 'shift' | 'fn', active: boolean) => void;
  snapshot: SimulatorSnapshot;
}) {
  return (
    <button
      type="button"
      onClick={(event) => {
        if (button.key === 'shift' || button.key === 'fn') {
          event.preventDefault();
          return;
        }
        const input = buttonInputForClick(button.key);
        if (input) dispatch(input);
      }}
      onMouseDown={() => {
        const modifier = modifierForButton(button.key);
        if (modifier) setModifier(modifier, true);
      }}
      onMouseUp={() => {
        const modifier = modifierForButton(button.key);
        if (modifier) setModifier(modifier, false);
      }}
      onMouseLeave={(event) => {
        const modifier = modifierForButton(button.key);
        if (modifier && (event.buttons & 1) === 1) setModifier(modifier, false);
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
    borderColor: lit
      ? `rgb(${Math.min(255, r + 64)}, ${Math.min(255, g + 64)}, ${Math.min(255, b + 64)})`
      : undefined,
    boxShadow: lit ? `0 0 14px rgba(${r}, ${g}, ${b}, 0.55)` : 'none',
  };
}
