import { useEffect, useMemo, useRef } from 'react';
import {
  OLED_HEIGHT,
  OLED_WIDTH,
  type RuntimeSnapshot,
} from '@octessera/device-contracts';
import { oledPanelCurrentFactor, toOledImage } from './oledImage';

const OLED_DISPLAY_SCALE = 2;
const OLED_DISPLAY_WIDTH = OLED_WIDTH * OLED_DISPLAY_SCALE;
const OLED_DISPLAY_HEIGHT = OLED_HEIGHT * OLED_DISPLAY_SCALE;

export function OledDisplay({
  frame,
  displayBrightness,
}: {
  frame: RuntimeSnapshot;
  displayBrightness: number;
}) {
  const panelCurrent = oledPanelCurrentFactor(displayBrightness);
  const { width, height, format, pixels } = frame.oled;
  const oledImage = useMemo(
    () => toOledImage({ width, height, format, pixels }, panelCurrent),
    [width, height, format, pixels, panelCurrent],
  );
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  useOledCanvas(canvasRef, oledImage);

  return (
    <section className="oled-wrap">
      <div className="oled-bezel">
        <div
          className="oled-panel"
          style={{
            width: OLED_DISPLAY_WIDTH,
            height: OLED_DISPLAY_HEIGHT,
            opacity: 1,
          }}
        >
          <canvas ref={canvasRef} className="oled-canvas" />
        </div>
      </div>
    </section>
  );
}

function useOledCanvas(
  ref: { current: HTMLCanvasElement | null },
  image: ImageData | null,
): void {
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    canvas.width = OLED_WIDTH;
    canvas.height = OLED_HEIGHT;
  }, [ref]);

  useEffect(() => {
    const ctx = ref.current?.getContext('2d');
    if (!ctx) return;
    if (image) {
      ctx.putImageData(image, 0, 0);
    } else {
      ctx.fillStyle = 'black';
      ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
    }
  }, [image, ref]);
}
