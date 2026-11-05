import { useEffect } from 'react';
import {
  createKeyboardMapper,
  shouldPreventKeyboardDefault,
} from '../runtime/inputAdapters/keyboardAdapter';
import type { DeviceInput } from '@octessera/device-contracts';
import type { createSimulatorRuntime } from '../runtime/simulatorRuntime';

type AppRuntime = ReturnType<typeof createSimulatorRuntime>;
type EncoderId = 'main' | `aux${number}`;
type AppSnapshot = ReturnType<AppRuntime['getSnapshot']>;

export function useRuntimeBindings(
  runtime: AppRuntime,
  setSnapshot: (snapshot: AppSnapshot) => void,
): void {
  useEffect(() => {
    const unsubscribeState = runtime.subscribe((snapshot) => {
      setSnapshot(snapshot);
    });
    runtime.start();
    return () => {
      unsubscribeState();
      runtime.stop();
    };
  }, [runtime, setSnapshot]);
}

export function useKeyboardBindings(
  runtime: AppRuntime,
  bumpDialPhase: (id: EncoderId | undefined, delta: number) => void,
): void {
  useEffect(() => {
    const mapper = createKeyboardMapper();
    const dispatch = (inputs: DeviceInput[]) => {
      for (const input of inputs) {
        if (input.type === 'encoder_turn') bumpDialPhase(input.id, input.delta);
        runtime.dispatch(input);
      }
    };

    const onKey = (event: KeyboardEvent) => {
      if (shouldPreventKeyboardDefault(event)) event.preventDefault();
      dispatch(mapper.keydown(event));
    };

    const onKeyUp = (event: KeyboardEvent) => {
      if (shouldPreventKeyboardDefault(event)) event.preventDefault();
      dispatch(mapper.keyup(event));
    };

    const onBlur = () => {
      dispatch(mapper.releaseAll());
      runtime.dispatch({ type: 'button_shift', pressed: false });
      runtime.dispatch({ type: 'button_fn', pressed: false });
    };

    window.addEventListener('keydown', onKey);
    window.addEventListener('keyup', onKeyUp);
    window.addEventListener('blur', onBlur);
    return () => {
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('blur', onBlur);
    };
  }, [bumpDialPhase, runtime]);
}

export function useDialDragBindings(
  dialDrag: { id: EncoderId; y: number; acc: number } | null,
  setDialDrag: (next: { id: EncoderId; y: number; acc: number } | null) => void,
  turnWithAcceleration: (
    id: EncoderId,
    delta: -1 | 1,
    magnitude: number,
  ) => void,
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
      turnWithAcceleration(
        dialDrag.id,
        nextAcc > 0 ? 1 : -1,
        Math.abs(nextAcc),
      );
      setDialDrag({ ...dialDrag, y: event.clientY, acc: 0 });
    };
    const onUp = () => setDialDrag(null);
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', onUp);
    return () => {
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', onUp);
    };
  }, [dialDrag, setDialDrag, turnWithAcceleration]);
}
