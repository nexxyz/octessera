import {
  BLACK_COLOR,
  OLED_HEIGHT,
  OLED_WIDTH,
  PULSES_COLOR,
  SPARKS_COLOR,
  SYSTEM_COLOR,
  TONES_COLOR,
  WHITE_COLOR,
  WORLDS_COLOR,
  type DisplayPaletteRgb
} from "@octessera/device-contracts";
import { rgb565ToCss } from "./oledImage";
import type { SemanticOledState } from "./OledDisplay";

export function drawSemanticOled(
  ctx: CanvasRenderingContext2D,
  semantic: SemanticOledState,
  regularSplashImage: HTMLImageElement | null,
  sepiaSplashImage: HTMLImageElement | null,
): void {
  ctx.clearRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
  ctx.fillStyle = css(BLACK_COLOR);
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);

  if (semantic.displayOff) return;

  if (semantic.splashText) {
    drawSplash(ctx, semantic.splashText, semantic.visibleFooterToast, regularSplashImage, sepiaSplashImage);
    return;
  }

  drawBackground(ctx);
  ctx.font = "8px monospace";
  ctx.textBaseline = "top";

  ctx.fillStyle = css(WHITE_COLOR);
  ctx.fillText(semantic.title, 5, 5, 94);

  ctx.fillStyle = semantic.cpuLoad >= 0.85 ? css(PULSES_COLOR) : semantic.cpuLoad >= 0.6 ? css(SPARKS_COLOR) : css(SYSTEM_COLOR);
  ctx.fillText("C", 117, 5);

  semantic.lines.forEach((line, index) => {
    const y = 18 + index * 13;
    const color = rgb565ToCss(typeof semantic.lineColors[index] === "number" ? semantic.lineColors[index]! : 0xffff);
    const selected = index === semantic.selectedRow;
    const bar = semantic.barValues[index] && typeof semantic.barValues[index] === "object"
      ? semantic.barValues[index] as { frac?: number; style?: string }
      : null;
    if (selected) {
      ctx.fillStyle = color;
      ctx.fillRect(3, y - 1, 122, 11);
    }
    if (bar) drawBar(ctx, y, Number(bar.frac ?? 0), bar.style);
    ctx.fillStyle = selected ? css(BLACK_COLOR) : color;
    ctx.fillText(line || " ", line.startsWith("  ") ? 4 : 6, y, 118);
  });
  if (semantic.scroll) drawScrollbar(ctx, semantic.scroll);

  drawFooter(ctx, semantic);
}

function drawFooter(ctx: CanvasRenderingContext2D, semantic: SemanticOledState): void {
  const footerY = 117;
  if (semantic.visibleFooterToast) {
    ctx.fillStyle = css(WHITE_COLOR);
    ctx.fillText(semantic.visibleFooterToast, 5, footerY, 90);
    return;
  }

  ctx.fillStyle = css(BLACK_COLOR);
  ctx.fillText(" ", 5, footerY, 90);

  ctx.fillStyle = semantic.transportIcon === "stop"
    ? css(PULSES_COLOR)
    : semantic.transportIcon === "pause"
      ? css(TONES_COLOR)
    : semantic.transportFlash === "measure"
      ? css(WORLDS_COLOR)
    : semantic.transportFlash === "beat"
        ? css(SPARKS_COLOR)
        : css(WHITE_COLOR);
  drawTransportIcon(ctx, semantic.transportIcon, 101, footerY + 1);
  if (semantic.eventDotOn) {
    ctx.fillStyle = semantic.eventDotSteal ? css(PULSES_COLOR) : css(WHITE_COLOR);
    ctx.beginPath();
    ctx.arc(121, footerY + 4, 3, 0, Math.PI * 2);
    ctx.fill();
  }
}

function drawScrollbar(ctx: CanvasRenderingContext2D, scroll: { offset: number; totalRows: number; visibleRows: number }): void {
  const bodyTop = 18;
  const bodyHeight = 7 * 13 - 3;
  const width = 2;
  const x = OLED_WIDTH - width - 1;
  const thumbHeight = Math.max(6, Math.round((scroll.visibleRows / scroll.totalRows) * bodyHeight));
  const maxOffset = Math.max(1, scroll.totalRows - scroll.visibleRows);
  const maxThumbY = bodyTop + bodyHeight - thumbHeight;
  const y = bodyTop + Math.round((Math.min(scroll.offset, maxOffset) / maxOffset) * (maxThumbY - bodyTop));
  ctx.fillStyle = rgba(SYSTEM_COLOR, 0.28);
  ctx.fillRect(x, bodyTop, width, bodyHeight);
  ctx.fillStyle = css(SYSTEM_COLOR);
  ctx.fillRect(x, y, width, thumbHeight);
}

function drawBackground(ctx: CanvasRenderingContext2D): void {
  ctx.fillStyle = css(BLACK_COLOR);
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
}

function drawBar(ctx: CanvasRenderingContext2D, y: number, frac: number, style?: string): void {
  const markerPct = Math.max(0, Math.min(1, frac));
  if (style === "marker") {
    const x = 3 + Math.round(markerPct * 122);
    ctx.fillStyle = rgba(WHITE_COLOR, 0.72);
    ctx.fillRect(Math.max(3, x - 1), y - 1, 2, 11);
    return;
  }
  ctx.fillStyle = rgba(WHITE_COLOR, 0.28);
  ctx.fillRect(3, y - 1, Math.round(markerPct * 122), 11);
}

function drawSplash(
  ctx: CanvasRenderingContext2D,
  splashText: string,
  toast: string,
  regularSplashImage: HTMLImageElement | null,
  sepiaSplashImage: HTMLImageElement | null,
): void {
  const sepia = splashText === "sleep" || splashText === "shutdown";
  ctx.fillStyle = css(BLACK_COLOR);
  ctx.fillRect(0, 0, OLED_WIDTH, OLED_HEIGHT);
  const logo = sepia ? sepiaSplashImage : regularSplashImage;
  if (logo) {
    ctx.drawImage(logo, 0, 0, OLED_WIDTH, OLED_HEIGHT);
  }
  ctx.font = "8px monospace";
  ctx.textBaseline = "top";
  if (toast) {
    ctx.fillStyle = rgba(BLACK_COLOR, 0.72);
    ctx.fillRect(10, 104, 108, 14);
    ctx.fillStyle = css(SYSTEM_COLOR);
    ctx.fillText(toast.toUpperCase(), 13, 107, 102);
  }
}

function css(rgb: DisplayPaletteRgb): string {
  return `rgb(${rgb[0]}, ${rgb[1]}, ${rgb[2]})`;
}

function rgba(rgb: DisplayPaletteRgb, alpha: number): string {
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
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
