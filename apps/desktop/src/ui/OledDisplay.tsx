import { useEffect, useMemo, useRef, useState } from "react";
import { OLED_HEIGHT, OLED_WIDTH, type OledFrame, type RuntimeSnapshot } from "@cellsymphony/device-contracts";
import { saveFlashVisible } from "./saveFlash";

const REGULAR_SPLASH_LOGO = new URL("../../../../assets/cellSymphonyLogo128.png", import.meta.url).href;
const SEPIA_SPLASH_LOGO = new URL("../../../../assets/cellSymphonyLogoSepia128.png", import.meta.url).href;

type SemanticOledState = {
  displayOff: boolean;
  splashText: string;
  title: string;
  lines: string[];
  selectedRow: number;
  lineColors: number[];
  barValues: Array<{ frac?: number; style?: string } | null>;
  transportIcon: string;
  eventDotOn: boolean;
  transportFlash: string;
  visibleFooterToast: string;
  showSaveFlash: boolean;
  cpuLoad: number;
};

export function OledDisplay({ frame, displayBrightness }: { frame: RuntimeSnapshot; displayBrightness: number }) {
  const oledImage = useMemo(() => toOledImage(frame.oled), [frame.oled]);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const regularSplashImage = useImageAsset(REGULAR_SPLASH_LOGO);
  const sepiaSplashImage = useImageAsset(SEPIA_SPLASH_LOGO);
  const displayOff = Boolean((frame.display as any).off ?? false);
  const splashText = String((frame.display as any).splash ?? "");
  const selectedRow = Number((frame as any).selectedRow ?? -1);
  const lineColors = Array.isArray(frame.display.colors) ? frame.display.colors : [];
  const barValues = Array.isArray((frame.display as any).barValues) ? (frame.display as any).barValues : [];
  const transportIcon = String((frame as any).transportIcon ?? (frame.transport.playing ? "play" : "pause"));
  const eventDotOn = Boolean((frame as any).eventDotOn ?? false);
  const transportFlash = String((frame as any).transportFlash ?? "none");
  const autoSaveFlash = String(frame.settings?.autoSaveFlash ?? "none");
  const autoSaveFlashSerial = Number((frame.settings as any)?.autoSaveFlashSerial ?? 0);
  const footerToast = String((frame.display as any).toast ?? "");
  const [saveFlashStartedAt, setSaveFlashStartedAt] = useState<number | null>(autoSaveFlash === "flash" ? Date.now() : null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [visibleFooterToast, setVisibleFooterToast] = useState(footerToast);
  const cpuLoad = Number((frame as any).cpuLoadRatio ?? 0);

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

  const semantic = useMemo<SemanticOledState>(
    () => ({
      displayOff,
      splashText,
      title: frame.display.title,
      lines: frame.display.lines,
      selectedRow,
      lineColors,
      barValues,
      transportIcon,
      eventDotOn,
      transportFlash,
      visibleFooterToast: splashText === "startup" ? "Starting up, loading defaults" : visibleFooterToast,
      showSaveFlash: saveFlashVisible(saveFlashStartedAt, nowMs),
      cpuLoad,
    }),
    [
      cpuLoad,
      displayOff,
      eventDotOn,
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
      visibleFooterToast,
    ],
  );

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

function rgb565ToCss(value: number): string {
  const r5 = (value >> 11) & 0x1f;
  const g6 = (value >> 5) & 0x3f;
  const b5 = value & 0x1f;
  const r = (r5 << 3) | (r5 >> 2);
  const g = (g6 << 2) | (g6 >> 4);
  const b = (b5 << 3) | (b5 >> 2);
  return `rgb(${r}, ${g}, ${b})`;
}

function toOledImage(oledFrame: OledFrame | undefined): ImageData | null {
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

function drawSemanticOled(
  ctx: CanvasRenderingContext2D,
  semantic: SemanticOledState,
  regularSplashImage: HTMLImageElement | null,
  sepiaSplashImage: HTMLImageElement | null,
) {
  ctx.clearRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
  ctx.fillStyle = "#000000";
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);

  if (semantic.displayOff) {
    return;
  }

  if (semantic.splashText) {
    drawSplash(ctx, semantic.splashText, semantic.visibleFooterToast, regularSplashImage, sepiaSplashImage);
    return;
  }

  drawBackground(ctx);
  ctx.font = "8px monospace";
  ctx.textBaseline = "top";

  ctx.fillStyle = "#ffffff";
  ctx.fillText(semantic.title, 5, 5, 94);

  ctx.fillStyle = semantic.showSaveFlash ? "#fff3b0" : "#334433";
  ctx.fillText("S", 108, 5);
  ctx.fillStyle = semantic.cpuLoad >= 0.85 ? "#ff6666" : semantic.cpuLoad >= 0.6 ? "#ffd166" : "#335544";
  ctx.fillText("C", 117, 5);

  semantic.lines.forEach((line, index) => {
    const y = 18 + index * 13;
    const color = rgb565ToCss(typeof semantic.lineColors[index] === "number" ? semantic.lineColors[index]! : 0xffff);
    const selected = index === semantic.selectedRow;
    const bar = semantic.barValues[index] && typeof semantic.barValues[index] === "object"
      ? semantic.barValues[index] as { frac?: number; style?: string }
      : null;
    if (bar) drawBar(ctx, y, Number(bar.frac ?? 0), bar.style);
    if (selected) {
      ctx.fillStyle = color;
      ctx.fillRect(3, y - 1, 122, 11);
    }
    ctx.fillStyle = selected ? "#04120d" : color;
    ctx.fillText(line || " ", line.startsWith("  ") ? 4 : 6, y, 118);
  });

  const footerY = 117;
  ctx.fillStyle = semantic.visibleFooterToast ? "#d7ffe8" : "#334433";
  ctx.fillText(semantic.visibleFooterToast || " ", 5, footerY, 90);

  ctx.fillStyle = semantic.transportFlash === "measure" ? "#ff3333" : semantic.transportFlash === "beat" ? "#33ff66" : "#d7ffe8";
  drawTransportIcon(ctx, semantic.transportIcon, 101, footerY + 1);
  if (semantic.eventDotOn) {
    ctx.fillStyle = "#ffffff";
    ctx.beginPath();
    ctx.arc(121, footerY + 4, 3, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawBackground(ctx: CanvasRenderingContext2D) {
  const gradient = ctx.createRadialGradient(50, 10, 8, 64, 64, 110);
  gradient.addColorStop(0, "rgba(26,65,52,0.35)");
  gradient.addColorStop(1, "rgba(0,0,0,0.95)");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
  ctx.fillStyle = "rgba(120,247,171,0.04)";
  for (let y = 0; y < OLED_HEIGHT; y += 3) {
    ctx.fillRect(0, y, OLED_WIDTH, 1);
  }
}

function drawBar(ctx: CanvasRenderingContext2D, y: number, frac: number, style?: string) {
  const markerPct = Math.max(0, Math.min(1, frac));
  if (style === "marker") {
    const x = 3 + Math.round(markerPct * 122);
    ctx.fillStyle = "rgba(215,255,232,0.72)";
    ctx.fillRect(Math.max(3, x - 1), y - 1, 2, 11);
    return;
  }
  ctx.fillStyle = "rgba(215,255,232,0.28)";
  ctx.fillRect(3, y - 1, Math.round(markerPct * 122), 11);
}

function drawSplash(
  ctx: CanvasRenderingContext2D,
  splashText: string,
  toast: string,
  regularSplashImage: HTMLImageElement | null,
  sepiaSplashImage: HTMLImageElement | null,
) {
  const sepia = splashText === "sleep" || splashText === "shutdown";
  ctx.fillStyle = "#000000";
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
  const logo = sepia ? sepiaSplashImage : regularSplashImage;
  if (logo) {
    ctx.drawImage(logo, 0, 0, OLED_WIDTH, OLED_HEIGHT);
  }
  ctx.font = "8px monospace";
  ctx.textBaseline = "top";
  if (toast) {
    ctx.fillStyle = "rgba(0,0,0,0.72)";
    ctx.fillRect(10, 104, 108, 14);
    ctx.fillStyle = "#f2f5df";
    ctx.fillText(toast.toUpperCase(), 13, 107, 102);
  }
}

function drawTransportIcon(ctx: CanvasRenderingContext2D, icon: string, x: number, y: number): void {
  if (icon === "play") {
    ctx.beginPath();
    ctx.moveTo(x, y - 1);
    ctx.lineTo(x, y + 9);
    ctx.lineTo(x + 8, y + 4);
    ctx.closePath();
    ctx.fill();
    return;
  }
  if (icon === "stop") {
    ctx.fillRect(x, y, 8, 8);
    return;
  }
  ctx.fillRect(x, y, 2, 8);
  ctx.fillRect(x + 6, y, 2, 8);
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
