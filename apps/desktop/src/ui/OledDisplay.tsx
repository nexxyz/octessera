import { useEffect, useMemo, useRef, useState } from "react";
import { OLED_HEIGHT, OLED_WIDTH, type RuntimeSnapshot } from "@cellsymphony/device-contracts";
import { drawSemanticOled } from "./oledDraw";
import { toOledImage } from "./oledImage";
import { saveFlashVisible } from "./saveFlash";

const REGULAR_SPLASH_LOGO = new URL("../../../../assets/cellSymphonyLogo128.png", import.meta.url).href;
const SEPIA_SPLASH_LOGO = new URL("../../../../assets/cellSymphonyLogoSepia128.png", import.meta.url).href;

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
  showSaveFlash: boolean;
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
  const regularSplashImage = useImageAsset(REGULAR_SPLASH_LOGO);
  const sepiaSplashImage = useImageAsset(SEPIA_SPLASH_LOGO);
  const semantic = useSemanticOledState(frame, audioLoad);

  useOledCanvas(canvasRef, oledImage, semantic, regularSplashImage, sepiaSplashImage);

  return (
    <section className="oled-wrap">
      <div className="oled-bezel">
        <div className="oled-panel" style={{ width: OLED_WIDTH, height: OLED_HEIGHT, opacity: Math.max(0.2, displayBrightness / 100) }}>
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
  const autoSaveFlash = String(frame.settings?.autoSaveFlash ?? "none");
  const autoSaveFlashSerial = Number(frame.settings?.autoSaveFlashSerial ?? 0);
  const footerToast = String(frame.display.toast ?? "");
  const [saveFlashStartedAt, setSaveFlashStartedAt] = useState<number | null>(autoSaveFlash === "flash" ? Date.now() : null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [visibleFooterToast, setVisibleFooterToast] = useState(footerToast);
  const frameCpuLoad = Number(frame.cpuLoadRatio ?? 0);
  const audioCpuLoad = audioLoad?.voiceSteal
    ? Math.max(audioLoad.ratio, 0.85)
    : (audioLoad?.ratio ?? 0);
  const cpuLoad = Math.max(frameCpuLoad, audioCpuLoad);

  useEffect(() => {
    if (autoSaveFlash !== "flash") {
      setSaveFlashStartedAt(null);
      return;
    }
    const startedAt = Date.now();
    setSaveFlashStartedAt(startedAt);
    setNowMs(startedAt);
    const interval = window.setInterval(() => setNowMs(Date.now()), 100);
    const timeout = window.setTimeout(() => {
      window.clearInterval(interval);
      setNowMs(Date.now());
    }, 700);
    return () => {
      window.clearInterval(interval);
      window.clearTimeout(timeout);
    };
  }, [autoSaveFlash, autoSaveFlashSerial]);

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
      showSaveFlash: saveFlashVisible(saveFlashStartedAt, nowMs),
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
      nowMs,
      saveFlashStartedAt,
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
    canvas.width = OLED_WIDTH;
    canvas.height = OLED_HEIGHT;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    if (image) {
      ctx.putImageData(image, 0, 0);
      return;
    }
    drawSemanticOled(ctx, semantic, regularSplashImage, sepiaSplashImage);
  }, [image, ref, regularSplashImage, semantic, sepiaSplashImage]);
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
