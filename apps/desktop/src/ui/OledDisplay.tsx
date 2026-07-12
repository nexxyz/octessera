import { useEffect, useMemo, useRef, useState } from "react";
import { OLED_HEIGHT, OLED_WIDTH, type RuntimeSnapshot } from "@octessera/device-contracts";
import { drawSemanticOled } from "./oledDraw";
import { toOledImage } from "./oledImage";

const BOOT_SPLASH_LOGO = new URL("../../../../assets/octessera-pi-booting.png", import.meta.url).href;
const SLEEP_SHUTDOWN_SPLASH_LOGO = new URL("../../../../assets/octessera-pi-shutdown.png", import.meta.url).href;
const OLED_DISPLAY_SCALE = 2;
const OLED_DISPLAY_WIDTH = OLED_WIDTH * OLED_DISPLAY_SCALE;
const OLED_DISPLAY_HEIGHT = OLED_HEIGHT * OLED_DISPLAY_SCALE;

export type SemanticOledState = {
  displayOff: boolean;
  splashText: string;
  title: string;
  lines: string[];
  selectedRow: number;
  lineColors: number[];
  barValues: Array<{ frac?: number; style?: string } | null>;
  scroll: { offset: number; totalRows: number; visibleRows: number } | null;
  transportIcon: string;
  eventDotOn: boolean;
  eventDotSteal: boolean;
  transportFlash: string;
  visibleFooterToast: string;
  cpuLoad: number;
};

export function OledDisplay({
  audioLoad,
  displayBrightness,
  frame
}: {
  audioLoad?: { ratio: number; voiceSteal: boolean };
  displayBrightness: number;
  frame: RuntimeSnapshot;
}) {
  const oledImage = useMemo(() => toOledImage(frame.oled), [frame.oled]);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const regularSplashImage = useImageAsset(BOOT_SPLASH_LOGO);
  const sepiaSplashImage = useImageAsset(SLEEP_SHUTDOWN_SPLASH_LOGO);
  const semantic = useSemanticOledState(frame, audioLoad);

  useOledCanvas(canvasRef, oledImage, semantic, regularSplashImage, sepiaSplashImage);

  return (
    <section className="oled-wrap">
      <div className="oled-bezel">
        <div
          className="oled-panel"
          style={{
            width: OLED_DISPLAY_WIDTH,
            height: OLED_DISPLAY_HEIGHT,
            opacity: Math.max(0.2, displayBrightness / 100),
          }}
        >
          <canvas ref={canvasRef} className="oled-canvas" />
        </div>
      </div>
    </section>
  );
}

function useSemanticOledState(
  frame: RuntimeSnapshot,
  audioLoad?: { ratio: number; voiceSteal: boolean }
): SemanticOledState {
  const displayOff = Boolean(frame.display.off ?? false);
  const splashText = String(frame.display.splash ?? "");
  const selectedRow = Number(frame.selectedRow ?? -1);
  const lineColors = Array.isArray(frame.display.colors) ? frame.display.colors : [];
  const barValues = Array.isArray(frame.display.barValues) ? frame.display.barValues : [];
  const scrollOffset = Number(frame.display.scrollOffset ?? 0);
  const totalRows = Number(frame.display.totalRows ?? 0);
  const visibleRows = Number(frame.display.visibleRows ?? 0);
  const scroll = totalRows > visibleRows && visibleRows > 0
    ? { offset: Math.max(0, scrollOffset), totalRows, visibleRows }
    : null;
  const transportIcon = String(frame.transportIcon ?? (frame.transport.playing ? "play" : "pause"));
  const eventDotOn = Boolean(frame.eventDotOn ?? false);
  const eventDotSteal = audioLoad?.voiceSteal === true;
  const transportFlash = String(frame.transportFlash ?? "none");
  const footerToast = String(frame.display.toast ?? "");
  const [visibleFooterToast, setVisibleFooterToast] = useState(footerToast);
  const frameCpuLoad = Number(frame.cpuLoadRatio ?? 0);
  const audioCpuLoad = audioLoad?.voiceSteal
    ? Math.max(audioLoad.ratio, 0.85)
    : (audioLoad?.ratio ?? 0);
  const cpuLoad = Math.max(frameCpuLoad, audioCpuLoad);

  useEffect(() => {
    if (!footerToast) {
      setVisibleFooterToast("");
      return;
    }
    setVisibleFooterToast(footerToast);
    const timeout = window.setTimeout(() => {
      setVisibleFooterToast((current) => (current === footerToast ? "" : current));
    }, 1800);
    return () => window.clearTimeout(timeout);
  }, [footerToast]);

  return useMemo<SemanticOledState>(
    () => ({
      displayOff,
      splashText,
      title: frame.display.title,
      lines: frame.display.lines,
      selectedRow,
      lineColors,
      barValues,
      scroll,
      transportIcon,
      eventDotOn,
      eventDotSteal,
      transportFlash,
      visibleFooterToast: splashText === "startup" ? "Starting up, loading defaults" : visibleFooterToast,
      cpuLoad,
    }),
    [
      cpuLoad,
      displayOff,
      eventDotOn,
      eventDotSteal,
      frame.display.lines,
      frame.display.title,
      lineColors,
      selectedRow,
      splashText,
      transportFlash,
      transportIcon,
      barValues,
      scroll,
      visibleFooterToast,
    ],
  );
}

function useOledCanvas(
  ref: { current: HTMLCanvasElement | null },
  image: ImageData | null,
  semantic: SemanticOledState,
  regularSplashImage: HTMLImageElement | null,
  sepiaSplashImage: HTMLImageElement | null,
): void {
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    canvas.width = OLED_DISPLAY_WIDTH;
    canvas.height = OLED_DISPLAY_HEIGHT;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, OLED_DISPLAY_WIDTH, OLED_DISPLAY_HEIGHT);

    const sourceCanvas = document.createElement("canvas");
    sourceCanvas.width = OLED_WIDTH;
    sourceCanvas.height = OLED_HEIGHT;
    const sourceCtx = sourceCanvas.getContext("2d");
    if (!sourceCtx) return;
    sourceCtx.imageSmoothingEnabled = false;

    if (image) {
      sourceCtx.putImageData(image, 0, 0);
    } else {
      drawSemanticOled(sourceCtx, semantic, regularSplashImage, sepiaSplashImage);
    }
    const sourceImage = sourceCtx.getImageData(0, 0, OLED_WIDTH, OLED_HEIGHT);
    ctx.putImageData(doubleImageData(sourceImage), 0, 0);
  }, [image, ref, regularSplashImage, semantic, sepiaSplashImage]);
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
      writePixel(targetData, targetWidth, targetX, targetY, sourceData, sourceIndex);
      writePixel(targetData, targetWidth, targetX + 1, targetY, sourceData, sourceIndex);
      writePixel(targetData, targetWidth, targetX, targetY + 1, sourceData, sourceIndex);
      writePixel(targetData, targetWidth, targetX + 1, targetY + 1, sourceData, sourceIndex);
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

function useImageAsset(src: string): HTMLImageElement | null {
  const [image, setImage] = useState<HTMLImageElement | null>(null);

  useEffect(() => {
    const next = new Image();
    next.src = src;
    const handleLoad = () => setImage(next);
    next.addEventListener("load", handleLoad);
    if (next.complete) setImage(next);
    return () => {
      next.removeEventListener("load", handleLoad);
    };
  }, [src]);

  return image;
}
