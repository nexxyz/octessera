import type { DeviceInput } from '@octessera/device-contracts';
import { AUX_ENCODER_COUNT } from '@octessera/device-contracts';
import { useEffect, useRef, type CSSProperties } from 'react';
import type { SimulatorSnapshot } from '../runtime/types';
import type { EncoderId } from './appHooks';
import { OledDisplay } from './OledDisplay';

type ModifierKind = 'shift' | 'fn';

const WHEEL_PIXELS_PER_TURN = 100;

/**
 * Turns for one wheel event: a notch-sized event turns at once, while the small events of a
 * trackpad or smooth-scrolling wheel add up in `pending` pixels until they make a whole turn.
 */
export function wheelTurns(
  pending: number,
  deltaY: number,
  deltaMode: number,
): { turns: number; pending: number } {
  const pixels = deltaMode === 0 ? deltaY : deltaY * WHEEL_PIXELS_PER_TURN;
  if (Math.abs(pixels) >= WHEEL_PIXELS_PER_TURN)
    return { turns: Math.round(pixels / WHEEL_PIXELS_PER_TURN), pending: 0 };
  const total =
    Math.sign(pending) === -Math.sign(pixels) ? pixels : pending + pixels;
  const turns = Math.trunc(total / WHEEL_PIXELS_PER_TURN) || 0;
  return { turns, pending: total - turns * WHEEL_PIXELS_PER_TURN };
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
  const dialRef = useRef<HTMLDivElement>(null);
  const wheelPending = useRef(0);
  useEffect(() => {
    const dial = dialRef.current;
    if (!dial) return;
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      const { turns, pending } = wheelTurns(
        wheelPending.current,
        event.deltaY,
        event.deltaMode,
      );
      wheelPending.current = pending;
      const delta = turns > 0 ? 1 : -1;
      for (let i = 0; i < Math.abs(turns); i += 1)
        dispatch({ type: 'encoder_turn', delta, id });
    };
    dial.addEventListener('wheel', onWheel, { passive: false });
    return () => dial.removeEventListener('wheel', onWheel);
  }, [dispatch, id]);
  return (
    <div
      ref={dialRef}
      className="encoder-dial"
      onMouseDown={(event) => {
        event.preventDefault();
        setDialDrag({ id, y: event.clientY, acc: 0 });
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
        onClick={() => {
          dispatch({ type: 'encoder_press', id });
          dispatch({ type: 'encoder_release', id });
        }}
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
