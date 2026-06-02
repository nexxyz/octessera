import { useEffect, useMemo, useRef, useState } from "react";
import { GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";
import { GRID_DOMAIN, OLED_HEIGHT, OLED_WIDTH, PLATFORM_CAPS } from "@cellsymphony/platform-core";
import { mapKeyboardEventToInputAction, mapKeyboardKeyupToInputAction, shouldPreventKeyboardDefault } from "../runtime/inputAdapters/keyboardAdapter";
import { sendEventsToAudio } from "../runtime/outputAdapters/audioSink";
import { createSimulatorRuntime } from "../runtime/simulatorRuntime";
import { nativeAudioBridge } from "../audio/nativeAudioBridge";
import { createCoalescedAudioConfigSender } from "../audio/coalescedAudioConfig";

const runtime = createSimulatorRuntime();
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

export function App() {
  const [snapshot, setSnapshot] = useState(() => runtime.getSnapshot());
  const [paintMode, setPaintMode] = useState<boolean | null>(null);
  const [painted, setPainted] = useState<Set<string>>(new Set());
  const [dialDrag, setDialDrag] = useState<{ id: EncoderId; y: number; acc: number } | null>(null);
  const [dialPhase, setDialPhase] = useState<Record<string, number>>({ main: 0, aux1: 0, aux2: 0, aux3: 0, aux4: 0 });
  const lastPressedCell = useRef<{ x: number; y: number } | null>(null);
  const frame = snapshot.frame;
  const oledCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const lastVoiceStealingMode = useRef<string>("");
  const audioConfigSender = useRef<ReturnType<typeof createCoalescedAudioConfigSender> | null>(null);
  if (audioConfigSender.current === null) {
    audioConfigSender.current = createCoalescedAudioConfigSender((config) => nativeAudioBridge.setInstruments(config));
  }

  const oledFrame = frame.oled;
  const oledImage = useMemo(() => toOledImage(oledFrame), [oledFrame]);

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

  useRuntimeBindings(setSnapshot);
  useKeyboardBindings(bumpDialPhase);
  useOledCanvas(oledCanvasRef, oledImage);
  useDialDragBindings(dialDrag, setDialDrag, turnWithAcceleration);

  const audioConfig = useMemo(() => {
    const instruments = (snapshot as any).instruments ?? [];
    const mixer = (snapshot as any).mixer ?? { buses: [] };
    const panPositions = PLATFORM_CAPS.gridWidth;
    return { instruments, mixer, panPositions };
  }, [snapshot.instruments, snapshot.mixer]);

  useEffect(() => {
    audioConfigSender.current?.schedule(audioConfig);
  }, [audioConfig]);

  useEffect(() => () => audioConfigSender.current?.flush(), []);

  useEffect(() => {
    const mode = snapshot.voiceStealingMode ?? "balanced";
    if (mode === lastVoiceStealingMode.current) return;
    lastVoiceStealingMode.current = mode;
    void nativeAudioBridge.setRuntimePolicy({ voiceStealingMode: mode });
  }, [snapshot.voiceStealingMode]);

  return (
    <main className="app-shell" onMouseUp={handleMouseUp} onMouseLeave={handleMouseUp}>
      <header className="bar">Cell Symphony Hardware Simulator</header>
      <section className="panel-layout">
        <section className="control-grid">
          <article className="encoder-card sw1">
            <h3>{ENCODERS[0].label}</h3>
            <div
              className="encoder-dial"
              onMouseDown={(event) => {
                event.preventDefault();
                setDialDrag({ id: "main", y: event.clientY, acc: 0 });
              }}
              onWheel={(event) => {
                event.preventDefault();
                turnWithAcceleration("main", event.deltaY > 0 ? 1 : -1, Math.abs(event.deltaY));
              }}
            >
              <div className="encoder-ring" aria-hidden="true">
                {Array.from({ length: 8 }, (_, i) => (
                  <span
                    key={`main-tick-${i}`}
                    className={`encoder-tick ${i === (dialPhase.main ?? 0) ? "active" : ""}`}
                    style={{ transform: `translate(-50%, -50%) rotate(${i * 45}deg) translateY(-45px)` }}
                  />
                ))}
              </div>
              <button type="button" className="encoder-center" onClick={() => dispatch({ type: "encoder_press", id: "main" })}>
                Push
              </button>
            </div>
            <small>Menu Control</small>
          </article>

          <section className="oled-wrap">
            <div className="oled-bezel">
              <div className="oled-panel" style={{ width: OLED_WIDTH, height: OLED_HEIGHT, opacity: Math.max(0.2, snapshot.displayBrightness / 100) }}>
                <canvas ref={oledCanvasRef} className="oled-canvas" />
              </div>
            </div>
          </section>

          <article className="encoder-card sw2">
            <h3>{ENCODERS[1].label}</h3>
            <Dial id="aux1" phase={dialPhase.aux1 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
            <small>Reserved</small>
          </article>
          <article className="encoder-card sw3">
            <h3>{ENCODERS[2].label}</h3>
            <Dial id="aux2" phase={dialPhase.aux2 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
            <small>Reserved</small>
          </article>
          <section className="button-stack stack-a">
            {NEOKEY_BUTTONS.slice(0, 2).map((button) => (
              <NeoKey key={button.key} button={button} dispatch={dispatch} snapshot={snapshot} />
            ))}
          </section>

          <article className="encoder-card sw4">
            <h3>{ENCODERS[3].label}</h3>
            <Dial id="aux3" phase={dialPhase.aux3 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
            <small>Reserved</small>
          </article>
          <article className="encoder-card sw5">
            <h3>{ENCODERS[4].label}</h3>
            <Dial id="aux4" phase={dialPhase.aux4 ?? 0} dispatch={dispatch} setDialDrag={setDialDrag} turnWithAcceleration={turnWithAcceleration} />
            <small>Reserved</small>
          </article>
          <section className="button-stack stack-b">
            {NEOKEY_BUTTONS.slice(2).map((button) => (
              <NeoKey key={button.key} button={button} dispatch={dispatch} snapshot={snapshot} />
            ))}
          </section>
        </section>

        <section className="matrix-chassis" aria-label="8 by 8 matrix">
          <div className="matrix">
            {frame.leds.cells.map((cell, index) => {
              const x = index % GRID_WIDTH;
              const y = Math.floor(index / GRID_WIDTH);
              return (
                <button
                  key={`${x}-${y}`}
                  type="button"
                  aria-label={`Grid ${x},${y}`}
                  className="cell"
                  style={{ backgroundColor: `rgb(${cell.r}, ${cell.g}, ${cell.b})` }}
                  onMouseDown={() => {
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
                  }}
                  onMouseEnter={(event) => {
                    if (event.buttons !== 1) return;
                    if (frame.gridInteraction === "momentary") {
                      pressMomentaryCell(x, y);
                      return;
                    }
                    if (paintMode === null) return;
                    applyPaint(x, y, paintMode);
                  }}
                  onClick={(event) => event.preventDefault()}
                />
              );
            })}
          </div>
        </section>
      </section>

      <footer className="bar footer">Left/Right/Up/Down or Wheel: SW1 turn • Enter: SW1 press • Backspace: Back • Space: Play/Pause • Shift+Space: Stop</footer>
    </main>
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
  snapshot
}: {
  button: (typeof NEOKEY_BUTTONS)[number];
  dispatch: (input: DeviceInput) => void;
  snapshot: ReturnType<typeof runtime.getSnapshot>;
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
        if (button.key === "shift") runtime.dispatchAction({ type: "shift", active: true });
        if (button.key === "fn") runtime.dispatchAction({ type: "fn", active: true });
      }}
      onMouseUp={() => {
        if (button.key === "shift") runtime.dispatchAction({ type: "shift", active: false });
        if (button.key === "fn") runtime.dispatchAction({ type: "fn", active: false });
      }}
      onMouseLeave={() => {
        if (button.key === "shift") runtime.dispatchAction({ type: "shift", active: false });
        if (button.key === "fn") runtime.dispatchAction({ type: "fn", active: false });
      }}
      className={`neokey-${button.key} ${snapshot.neoKeyLeds[button.key]}`}
      style={{ opacity: Math.max(0.25, snapshot.buttonBrightness / 100) }}
    >
      {button.label}
    </button>
  );
}

function toOledImage(oledFrame: ReturnType<typeof runtime.getSnapshot>["frame"]["oled"]): ImageData | null {
  if (!oledFrame || oledFrame.format !== "rgb565be") return null;
  const w = oledFrame.width;
  const h = oledFrame.height;
  const data = new Uint8ClampedArray(w * h * 4);
  const px = oledFrame.pixels;
  for (let i = 0, j = 0; i < px.length; i += 2, j += 4) {
    const v = (px[i]! << 8) | px[i + 1]!;
    const r5 = (v >> 11) & 0x1f;
    const g6 = (v >> 5) & 0x3f;
    const b5 = v & 0x1f;
    data[j] = (r5 << 3) | (r5 >> 2);
    data[j + 1] = (g6 << 2) | (g6 >> 4);
    data[j + 2] = (b5 << 3) | (b5 >> 2);
    data[j + 3] = 255;
  }
  return new ImageData(data, w, h);
}

function useRuntimeBindings(setSnapshot: (snapshot: ReturnType<typeof runtime.getSnapshot>) => void): void {
  useEffect(() => {
    const unsubscribeState = runtime.subscribe(setSnapshot);
    const unsubscribeEvents = runtime.subscribeEvents((events) => {
      void sendEventsToAudio(events, runtime.getSnapshot().masterVolume);
    });
    runtime.start();
    return () => {
      unsubscribeState();
      unsubscribeEvents();
      runtime.stop();
    };
  }, [setSnapshot]);
}

function useKeyboardBindings(bumpDialPhase: (id: EncoderId | undefined, delta: -1 | 1) => void): void {
  useEffect(() => {
    // Track which keys are currently pressed to handle key repeats
    const pressedKeys = new Set<string>();

    const onKey = (event: KeyboardEvent) => {
      if (shouldPreventKeyboardDefault(event)) event.preventDefault();
      const action = mapKeyboardEventToInputAction(event);
      if (!action) return;

      // Handle hardware-like button keys with edge-only behavior
      const edgeOnlyKeys = new Set(["Shift", "Control", " ", "Enter", "Backspace", "Escape"]);
      if (edgeOnlyKeys.has(event.key)) {
        if (pressedKeys.has(event.key) || event.repeat) return;
        pressedKeys.add(event.key);
      }

      // Handle arrow keys - make them repeat like encoder turns
      if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
        // Arrow keys should not use this suppression if you want encoder-repeat behavior
        // They can use the browser's native repeat behavior instead
      }

      if (action.type === "device_input" && action.input.type === "encoder_turn") {
        bumpDialPhase(action.input.id, action.input.delta);
      }
      runtime.dispatchAction(action);
    };

    const onKeyUp = (event: KeyboardEvent) => {
      // Remove from pressed keys tracking
      pressedKeys.delete(event.key);

      const action = mapKeyboardKeyupToInputAction(event);
      if (action) runtime.dispatchAction(action);
    };

    const onBlur = () => {
      // Clear all held keys and timers on blur
      pressedKeys.clear();
      runtime.dispatchAction({ type: "shift", active: false });
      runtime.dispatchAction({ type: "fn", active: false });
    };

    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKeyUp);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("keyup", onKeyUp);
      window.removeEventListener("blur", onBlur);
    };
  }, [bumpDialPhase]);
}

function useOledCanvas(ref: { current: HTMLCanvasElement | null }, image: ImageData | null): void {
  useEffect(() => {
    if (!image) return;
    const canvas = ref.current;
    if (!canvas) return;
    canvas.width = image.width;
    canvas.height = image.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.putImageData(image, 0, 0);
  }, [image, ref]);
}

function useDialDragBindings(
  dialDrag: { id: EncoderId; y: number; acc: number } | null,
  setDialDrag: (next: { id: EncoderId; y: number; acc: number } | null) => void,
  turnWithAcceleration: (id: EncoderId, delta: -1 | 1, magnitude: number) => void
): void {
  useEffect(() => {
    if (!dialDrag) return;
    const onMove = (event: MouseEvent) => {
      const deltaY = dialDrag.y - event.clientY;
      const nextAcc = dialDrag.acc + deltaY;
      if (Math.abs(nextAcc) < 12) {
        setDialDrag({ ...dialDrag, y: event.clientY, acc: nextAcc });
        return;
      }
      turnWithAcceleration(dialDrag.id, nextAcc > 0 ? 1 : -1, Math.abs(nextAcc));
      setDialDrag({ ...dialDrag, y: event.clientY, acc: 0 });
    };
    const onUp = () => setDialDrag(null);
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [dialDrag, setDialDrag, turnWithAcceleration]);
}
