import { startTransition, useEffect, useMemo, useRef } from "react";
import { PAN_POSITION_COUNT } from "@cellsymphony/device-contracts";
import { nativeAudioBridge } from "../audio/nativeAudioBridge";
import { createCoalescedAudioConfigSender } from "../audio/coalescedAudioConfig";
import { mapKeyboardEventToInputAction, mapKeyboardKeyupToInputAction, shouldPreventKeyboardDefault } from "../runtime/inputAdapters/keyboardAdapter";
import type { createSimulatorRuntime } from "../runtime/simulatorRuntime";

type AppRuntime = ReturnType<typeof createSimulatorRuntime>;
type EncoderId = "main" | `aux${number}`;
type AppSnapshot = ReturnType<AppRuntime["getSnapshot"]>;

export function useRuntimeBindings(runtime: AppRuntime, setSnapshot: (snapshot: AppSnapshot) => void): void {
  useEffect(() => {
    const unsubscribeState = runtime.subscribe((snapshot) => {
      startTransition(() => setSnapshot(snapshot));
    });
    runtime.start();
    return () => {
      unsubscribeState();
      runtime.stop();
    };
  }, [runtime, setSnapshot]);
}

export function useKeyboardBindings(runtime: AppRuntime, bumpDialPhase: (id: EncoderId | undefined, delta: -1 | 1) => void): void {
  useEffect(() => {
    const pressedKeys = new Set<string>();

    const onKey = (event: KeyboardEvent) => {
      if (shouldPreventKeyboardDefault(event)) event.preventDefault();
      const action = mapKeyboardEventToInputAction(event);
      if (!action) return;

      const edgeOnlyKeys = new Set(["Shift", "Control", " ", "Enter", "Backspace", "Escape"]);
      if (edgeOnlyKeys.has(event.key)) {
        if (pressedKeys.has(event.key) || event.repeat) return;
        pressedKeys.add(event.key);
      }

      if (action.type === "device_input" && action.input.type === "encoder_turn") {
        bumpDialPhase(action.input.id, action.input.delta);
      }
      runtime.dispatchAction(action);
    };

    const onKeyUp = (event: KeyboardEvent) => {
      pressedKeys.delete(event.key);
      const action = mapKeyboardKeyupToInputAction(event);
      if (action) runtime.dispatchAction(action);
    };

    const onBlur = () => {
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
  }, [bumpDialPhase, runtime]);
}

export function useDialDragBindings(
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

export function useAudioConfigSync(snapshot: AppSnapshot): void {
  const lastVoiceStealingMode = useRef<string>("");
  const audioConfigSender = useRef<ReturnType<typeof createCoalescedAudioConfigSender> | null>(null);
  if (audioConfigSender.current === null) {
    audioConfigSender.current = createCoalescedAudioConfigSender((config) => nativeAudioBridge.setInstruments(config));
  }

  const audioConfig = useMemo(() => {
    const instruments = (snapshot as any).instruments ?? [];
    const mixer = (snapshot as any).mixer ?? { buses: [] };
    const panPositions = Number((snapshot as any).panPositions ?? PAN_POSITION_COUNT);
    return { instruments, mixer, panPositions, masterVolume: snapshot.masterVolume };
  }, [snapshot.audioConfigRevision, snapshot.instruments, snapshot.masterVolume, snapshot.mixer, snapshot.panPositions]);

  useEffect(() => {
    if (audioConfig.instruments.length === 0) return;
    audioConfigSender.current?.schedule(audioConfig);
  }, [audioConfig]);

  useEffect(() => () => audioConfigSender.current?.flush(), []);

  useEffect(() => {
    const mode = snapshot.voiceStealingMode ?? "balanced";
    if (mode === lastVoiceStealingMode.current) return;
    lastVoiceStealingMode.current = mode;
    void nativeAudioBridge.setRuntimePolicy({ voiceStealingMode: mode });
  }, [snapshot.voiceStealingMode]);
}
