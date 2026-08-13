import { useEffect, useMemo, useRef } from 'react';
import {
  OLED_HEIGHT,
  OLED_WIDTH,
  type RuntimeSnapshot,
} from '@octessera/device-contracts';
import { toOledImage } from './oledImage';

const OLED_DISPLAY_SCALE = 2;
const OLED_DISPLAY_WIDTH = OLED_WIDTH * OLED_DISPLAY_SCALE;
const OLED_DISPLAY_HEIGHT = OLED_HEIGHT * OLED_DISPLAY_SCALE;

export function OledDisplay({ frame }: { frame: RuntimeSnapshot }) {
  const oledImage = useMemo(() => toOledImage(frame.oled), [frame.oled]);
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
    canvas.width = OLED_DISPLAY_WIDTH;
    canvas.height = OLED_DISPLAY_HEIGHT;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.fillStyle = 'black';
    ctx.fillRect(0, 0, OLED_DISPLAY_WIDTH, OLED_DISPLAY_HEIGHT);

    const sourceCanvas = document.createElement('canvas');
    sourceCanvas.width = OLED_WIDTH;
    sourceCanvas.height = OLED_HEIGHT;
    const sourceCtx = sourceCanvas.getContext('2d');
    if (!sourceCtx) return;
    sourceCtx.imageSmoothingEnabled = false;

    if (!image) return;
    sourceCtx.putImageData(image, 0, 0);
    const sourceImage = sourceCtx.getImageData(0, 0, OLED_WIDTH, OLED_HEIGHT);
    ctx.putImageData(doubleImageData(sourceImage), 0, 0);
  }, [image, ref]);
}

function doubleImageData(source: ImageData): ImageData {
  const target = new ImageData(source.width * 2, source.height * 2);
  const sourceData = source.data;
  const targetData = target.data;
  const targetWidth = target.width;

  for (let y = 0; y < source.height; y += 1) {
    for (let x = 0; x < source.width; x += 1) {
      const sourceIndex = (y * source.width + x) * 4;
      const targetX = x * 2;
      const targetY = y * 2;
      writePixel(
        targetData,
        targetWidth,
        targetX,
        targetY,
        sourceData,
        sourceIndex,
      );
      writePixel(
        targetData,
        targetWidth,
        targetX + 1,
        targetY,
        sourceData,
        sourceIndex,
      );
      writePixel(
        targetData,
        targetWidth,
        targetX,
        targetY + 1,
        sourceData,
        sourceIndex,
      );
      writePixel(
        targetData,
        targetWidth,
        targetX + 1,
        targetY + 1,
        sourceData,
        sourceIndex,
      );
    }
  }

  return target;
}

function writePixel(
  targetData: Uint8ClampedArray,
  targetWidth: number,
  x: number,
  y: number,
  sourceData: Uint8ClampedArray,
  sourceIndex: number,
): void {
  const targetIndex = (y * targetWidth + x) * 4;
  targetData[targetIndex] = sourceData[sourceIndex];
  targetData[targetIndex + 1] = sourceData[sourceIndex + 1];
  targetData[targetIndex + 2] = sourceData[sourceIndex + 2];
  targetData[targetIndex + 3] = sourceData[sourceIndex + 3];
}
