import type { OledFrame } from "@cellsymphony/device-contracts";
import type { BarValue } from "./platformTypes";
import { nowMs } from "./timing";

export type OledRenderState = {
  lines: string[]; // already clamped to OLED_TEXT_LINES
  lineColors?: number[]; // RGB565 color per line (non-selected items)
  barValues?: (BarValue | null)[]; // bar metadata per line, null = no bar
  splash?: { pixelsRgb565be: Uint8Array; topText: string; bottomText: string | null };
  off?: boolean;
  transportIcon?: "play" | "pause" | "stop";
  transportFlash?: "none" | "beat" | "measure";
  eventDotOn?: boolean;
  audioLoadIndicator?: "yellow" | "red";
  toast?: string | null;
  toastStartedAtMs?: number;
  renderNowMs?: number;
  autoSaveFlash?: "none" | "flash";
};

const OLED_W = 128;
const OLED_H = 128;
const OLED_TEXT_COLUMNS = 20;
const OLED_TEXT_LINES = 8;

// 5x7 font (plus 1px spacer column), ASCII 32..127.
// Each character is 5 bytes, LSB at top.
// Source: classic public-domain 5x7 font table.
const FONT_5X7: number[] = [
  0x00, 0x00, 0x00, 0x00, 0x00, // space
  0x00, 0x00, 0x5f, 0x00, 0x00, // !
  0x00, 0x07, 0x00, 0x07, 0x00, // "
  0x14, 0x7f, 0x14, 0x7f, 0x14, // #
  0x24, 0x2a, 0x7f, 0x2a, 0x12, // $
  0x23, 0x13, 0x08, 0x64, 0x62, // %
  0x36, 0x49, 0x55, 0x22, 0x50, // &
  0x00, 0x05, 0x03, 0x00, 0x00, // '
  0x00, 0x1c, 0x22, 0x41, 0x00, // (
  0x00, 0x41, 0x22, 0x1c, 0x00, // )
  0x14, 0x08, 0x3e, 0x08, 0x14, // *
  0x08, 0x08, 0x3e, 0x08, 0x08, // +
  0x00, 0x50, 0x30, 0x00, 0x00, // ,
  0x08, 0x08, 0x08, 0x08, 0x08, // -
  0x00, 0x60, 0x60, 0x00, 0x00, // .
  0x20, 0x10, 0x08, 0x04, 0x02, // /
  0x3e, 0x51, 0x49, 0x45, 0x3e, // 0
  0x00, 0x42, 0x7f, 0x40, 0x00, // 1
  0x42, 0x61, 0x51, 0x49, 0x46, // 2
  0x21, 0x41, 0x45, 0x4b, 0x31, // 3
  0x18, 0x14, 0x12, 0x7f, 0x10, // 4
  0x27, 0x45, 0x45, 0x45, 0x39, // 5
  0x3c, 0x4a, 0x49, 0x49, 0x30, // 6
  0x01, 0x71, 0x09, 0x05, 0x03, // 7
  0x36, 0x49, 0x49, 0x49, 0x36, // 8
  0x06, 0x49, 0x49, 0x29, 0x1e, // 9
  0x00, 0x36, 0x36, 0x00, 0x00, // :
  0x00, 0x56, 0x36, 0x00, 0x00, // ;
  0x08, 0x14, 0x22, 0x41, 0x00, // <
  0x14, 0x14, 0x14, 0x14, 0x14, // =
  0x00, 0x41, 0x22, 0x14, 0x08, // >
  0x02, 0x01, 0x51, 0x09, 0x06, // ?
  0x32, 0x49, 0x79, 0x41, 0x3e, // @
  0x7e, 0x11, 0x11, 0x11, 0x7e, // A
  0x7f, 0x49, 0x49, 0x49, 0x36, // B
  0x3e, 0x41, 0x41, 0x41, 0x22, // C
  0x7f, 0x41, 0x41, 0x22, 0x1c, // D
  0x7f, 0x49, 0x49, 0x49, 0x41, // E
  0x7f, 0x09, 0x09, 0x09, 0x01, // F
  0x3e, 0x41, 0x49, 0x49, 0x7a, // G
  0x7f, 0x08, 0x08, 0x08, 0x7f, // H
  0x00, 0x41, 0x7f, 0x41, 0x00, // I
  0x20, 0x40, 0x41, 0x3f, 0x01, // J
  0x7f, 0x08, 0x14, 0x22, 0x41, // K
  0x7f, 0x40, 0x40, 0x40, 0x40, // L
  0x7f, 0x02, 0x0c, 0x02, 0x7f, // M
  0x7f, 0x04, 0x08, 0x10, 0x7f, // N
  0x3e, 0x41, 0x41, 0x41, 0x3e, // O
  0x7f, 0x09, 0x09, 0x09, 0x06, // P
  0x3e, 0x41, 0x51, 0x21, 0x5e, // Q
  0x7f, 0x09, 0x19, 0x29, 0x46, // R
  0x46, 0x49, 0x49, 0x49, 0x31, // S
  0x01, 0x01, 0x7f, 0x01, 0x01, // T
  0x3f, 0x40, 0x40, 0x40, 0x3f, // U
  0x1f, 0x20, 0x40, 0x20, 0x1f, // V
  0x3f, 0x40, 0x38, 0x40, 0x3f, // W
  0x63, 0x14, 0x08, 0x14, 0x63, // X
  0x07, 0x08, 0x70, 0x08, 0x07, // Y
  0x61, 0x51, 0x49, 0x45, 0x43, // Z
  0x00, 0x7f, 0x41, 0x41, 0x00, // [
  0x02, 0x04, 0x08, 0x10, 0x20, // \
  0x00, 0x41, 0x41, 0x7f, 0x00, // ]
  0x04, 0x02, 0x01, 0x02, 0x04, // ^
  0x40, 0x40, 0x40, 0x40, 0x40, // _
  0x00, 0x01, 0x02, 0x04, 0x00, // `
  0x20, 0x54, 0x54, 0x54, 0x78, // a
  0x7f, 0x48, 0x44, 0x44, 0x38, // b
  0x38, 0x44, 0x44, 0x44, 0x20, // c
  0x38, 0x44, 0x44, 0x48, 0x7f, // d
  0x38, 0x54, 0x54, 0x54, 0x18, // e
  0x08, 0x7e, 0x09, 0x01, 0x02, // f
  0x0c, 0x52, 0x52, 0x52, 0x3e, // g
  0x7f, 0x08, 0x04, 0x04, 0x78, // h
  0x00, 0x44, 0x7d, 0x40, 0x00, // i
  0x20, 0x40, 0x44, 0x3d, 0x00, // j
  0x7f, 0x10, 0x28, 0x44, 0x00, // k
  0x00, 0x41, 0x7f, 0x40, 0x00, // l
  0x7c, 0x04, 0x18, 0x04, 0x78, // m
  0x7c, 0x08, 0x04, 0x04, 0x78, // n
  0x38, 0x44, 0x44, 0x44, 0x38, // o
  0x7c, 0x14, 0x14, 0x14, 0x08, // p
  0x08, 0x14, 0x14, 0x18, 0x7c, // q
  0x7c, 0x08, 0x04, 0x04, 0x08, // r
  0x48, 0x54, 0x54, 0x54, 0x20, // s
  0x04, 0x3f, 0x44, 0x40, 0x20, // t
  0x3c, 0x40, 0x40, 0x20, 0x7c, // u
  0x1c, 0x20, 0x40, 0x20, 0x1c, // v
  0x3c, 0x40, 0x30, 0x40, 0x3c, // w
  0x44, 0x28, 0x10, 0x28, 0x44, // x
  0x0c, 0x50, 0x50, 0x50, 0x3c, // y
  0x44, 0x64, 0x54, 0x4c, 0x44, // z
  0x00, 0x08, 0x36, 0x41, 0x00, // {
  0x00, 0x00, 0x7f, 0x00, 0x00, // |
  0x00, 0x41, 0x36, 0x08, 0x00, // }
  0x08, 0x08, 0x2a, 0x1c, 0x08, // ~
  0x08, 0x1c, 0x2a, 0x08, 0x08 // DEL (unused)
];

function rgb565ToBytesBE(v: number): [number, number] {
  return [(v >> 8) & 0xff, v & 0xff];
}

function setPixel565be(buf: Uint8Array, x: number, y: number, v: number) {
  if (x < 0 || y < 0 || x >= OLED_W || y >= OLED_H) return;
  const idx = (y * OLED_W + x) * 2;
  buf[idx] = (v >> 8) & 0xff;
  buf[idx + 1] = v & 0xff;
}

type Rect = { x: number; y: number; w: number; h: number };

function fillRect(buf: Uint8Array, rect: Rect, v: number) {
  for (let yy = 0; yy < rect.h; yy += 1) {
    for (let xx = 0; xx < rect.w; xx += 1) {
      setPixel565be(buf, rect.x + xx, rect.y + yy, v);
    }
  }
}

type TextStyle = { fg: number; bg: number | null };
type DrawCharParams = { pos: { x: number; y: number }; ch: string; style: TextStyle };

function drawChar(buf: Uint8Array, params: DrawCharParams) {
  const { pos, ch, style } = params;
  const code = ch.charCodeAt(0);
  const idx = (code - 32) * 5;
  if (idx < 0 || idx + 4 >= FONT_5X7.length) {
    // draw as space
    if (style.bg !== null) fillRect(buf, { x: pos.x, y: pos.y, w: 6, h: 8 }, style.bg);
    return;
  }
  if (style.bg !== null) fillRect(buf, { x: pos.x, y: pos.y, w: 6, h: 8 }, style.bg);
  for (let col = 0; col < 5; col += 1) {
    const bits = FONT_5X7[idx + col] ?? 0;
    for (let row = 0; row < 7; row += 1) {
      if (bits & (1 << row)) {
        setPixel565be(buf, pos.x + col, pos.y + row, style.fg);
      }
    }
  }
}

type DrawTextParams = { pos: { x: number; y: number }; text: string; style: TextStyle };

function drawText(buf: Uint8Array, params: DrawTextParams) {
  const { pos, text, style } = params;
  for (let i = 0; i < text.length; i += 1) {
    drawChar(buf, { pos: { x: pos.x + i * 6, y: pos.y }, ch: text[i] ?? " ", style });
  }
}

function decodeSelected(line: string): { selected: boolean; text: string } {
  if (line.startsWith("@@")) return { selected: true, text: line.slice(2) };
  return { selected: false, text: line };
}

function drawTransport(
  buf: Uint8Array,
  icon: "play" | "pause" | "stop",
  flash: "none" | "beat" | "measure",
  eventDotOn: boolean
) {
  // Bottom-right 14x10 region.
  const x0 = 128 - 14;
  const y0 = 128 - 10;
  const base = 0xffff;
  const playColor = flash === "beat" ? 0x07e0 : flash === "measure" ? 0xf800 : 0xffff;
  const fg = icon === "play" ? playColor : base;
  const dim = 0x39c7; // mid gray

  // Icon (fit within 9x8, leaving space for dot).
  if (icon === "play") {
    // Right-pointing triangle centered vertically.
    // Base is on the left, apex on the right.
    const cx = x0 + 0; // shift left
    const cy = y0 + 4;
    for (let dx = 0; dx <= 6; dx += 1) {
      const span = 6 - dx;
      for (let dy = -span; dy <= span; dy += 1) {
        setPixel565be(buf, cx + dx, cy + dy, fg);
      }
    }
  } else if (icon === "stop") {
    fillRect(buf, { x: x0 + 0, y: y0 + 2, w: 7, h: 7 }, fg);
  } else {
    // pause
    fillRect(buf, { x: x0 + 0, y: y0 + 2, w: 2, h: 7 }, fg);
    fillRect(buf, { x: x0 + 4, y: y0 + 2, w: 2, h: 7 }, fg);
  }

  // event dot
  const dotX = x0 + 11;
  const dotY = y0 + 4;
  const dotOn = 0xffe0; // bright yellow
  const dotOff = dim;
  fillRect(buf, { x: dotX - 1, y: dotY - 1, w: 3, h: 3 }, eventDotOn ? dotOn : dotOff);
}

function drawAudioLoadIndicator(buf: Uint8Array, indicator: "yellow" | "red") {
  const color = indicator === "red" ? 0xf800 : 0xffe0;
  fillRect(buf, { x: OLED_W - 7, y: 3, w: 4, h: 4 }, color);
}

function toastWindow(message: string, now: number, startedAtMs: number, maxChars: number): string {
  if (message.length <= maxChars) return message;
  const holdMs = 700;
  const scrollStepMs = 120;
  const totalScroll = message.length - maxChars;
  const totalScrollMs = totalScroll * scrollStepMs;
  const elapsed = now - startedAtMs;
  if (elapsed < holdMs) return message.slice(0, maxChars);
  if (elapsed >= holdMs + totalScrollMs) return message.slice(totalScroll);
  const offset = Math.min(Math.floor((elapsed - holdMs) / scrollStepMs), totalScroll);
  return message.slice(offset, offset + maxChars);
}

export function renderOledFrame(state: OledRenderState): OledFrame {
  const buf = new Uint8Array(OLED_W * OLED_H * 2);

  // Default clear to black.
  // If splash/off is requested, handle early.
  if (state.off) {
    return { width: 128, height: 128, format: "rgb565be", pixels: buf };
  }

  if (state.splash) {
    buf.set(state.splash.pixelsRgb565be);
    const top = (state.splash.topText ?? "").slice(0, OLED_TEXT_COLUMNS);
    if (top.trim().length > 0) drawText(buf, { pos: { x: 4, y: 2 }, text: top, style: { fg: 0xffff, bg: null } });
    const bottom = state.splash.bottomText ? state.splash.bottomText.slice(0, OLED_TEXT_COLUMNS) : "";
    if (bottom.trim().length > 0) drawText(buf, { pos: { x: 4, y: OLED_H - 10 }, text: bottom, style: { fg: 0xffff, bg: null } });
    if (state.autoSaveFlash === "flash") {
      drawText(buf, { pos: { x: OLED_W - 10, y: 2 }, text: "S", style: { fg: 0xffff, bg: null } });
    }
    return { width: 128, height: 128, format: "rgb565be", pixels: buf };
  }

  const fg = 0xffff;
  const invFg = 0x0000;
  const invBg = 0xffff;
  const normalStyleBase: TextStyle = { fg, bg: null };
  const selectedStyle: TextStyle = { fg: invFg, bg: null };

  const lineHeight = Math.floor(OLED_H / OLED_TEXT_LINES); // 16
  const xStart = 4;
  const barDim = 0x39c7; // mid gray for bar fill
  const prefixChars = 3; // " *" or "  "
  const barCharWidth = 12; // 12 character widths for the bar area
  for (let i = 0; i < OLED_TEXT_LINES; i += 1) {
    const line = state.lines[i] ?? "";
    const { selected, text } = decodeSelected(line);
    const y = i * lineHeight + 4;
    // Clamp to 20 cols.
    const clipped = text.slice(0, OLED_TEXT_COLUMNS);
    if (selected) {
      const bgColor = state.lineColors?.[i] ?? invBg; // Use section color as bg
      fillRect(buf, { x: 0, y: i * lineHeight, w: OLED_W, h: lineHeight }, bgColor);
      // Draw bar geometry before text so text renders on top
      if (state.barValues?.[i] != null) {
        const bar = state.barValues[i]!;
        const frac = Math.max(0, Math.min(1, bar.frac));
        const numChars = Math.max(0, Math.min(OLED_TEXT_COLUMNS - prefixChars, bar.numChars));
        const barX = xStart + (prefixChars + numChars) * 6;
        const barW = Math.min(barCharWidth, OLED_TEXT_COLUMNS - prefixChars - numChars) * 6;
        if (bar.style === "marker") {
          fillRect(buf, { x: barX, y: y + 3, w: barW, h: 1 }, barDim);
          const markerX = Math.max(barX, Math.min(barX + Math.max(0, barW - 2), barX + Math.round(frac * Math.max(0, barW - 1)) - 1));
          fillRect(buf, { x: markerX, y, w: 2, h: 7 }, barDim);
        } else {
          const fillW = Math.round(frac * barW);
          if (fillW > 0) fillRect(buf, { x: barX, y, w: fillW, h: 7 }, barDim);
          if (fillW < barW) fillRect(buf, { x: barX + fillW, y: y + 6, w: barW - fillW, h: 1 }, barDim);
        }
      }
      drawText(buf, { pos: { x: xStart, y }, text: clipped, style: selectedStyle }); // Black text
    } else {
      const lineColor = state.lineColors?.[i] ?? fg;
      drawText(buf, {
        pos: { x: xStart, y },
        text: clipped,
        style: lineColor === fg ? normalStyleBase : { fg: lineColor, bg: null }
      });
    }
  }

  if (state.transportIcon) {
    drawTransport(buf, state.transportIcon, state.transportFlash ?? "none", Boolean(state.eventDotOn));
  }

  if (state.autoSaveFlash === "flash") {
    drawText(buf, { pos: { x: OLED_W - 10, y: 4 }, text: "S", style: { fg: 0xffff, bg: null } });
  }

  if (state.audioLoadIndicator) drawAudioLoadIndicator(buf, state.audioLoadIndicator);

  if (state.toast) {
    // Reserve the rightmost area for transport indicator.
    const maxChars = 17; // 17*6 + xStart(4) ~= 106px
    const now = state.renderNowMs ?? nowMs();
    const startedAt = state.toastStartedAtMs ?? now;
    const msg = toastWindow(state.toast, now, startedAt, maxChars);
    drawText(buf, { pos: { x: 4, y: OLED_H - 10 }, text: msg, style: normalStyleBase });
  }

  return { width: 128, height: 128, format: "rgb565be", pixels: buf };
}
