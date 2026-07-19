import test from "node:test";
import assert from "node:assert/strict";
import { BLACK_COLOR } from "@octessera/device-contracts";
import { drawSemanticOled } from "../src/ui/oledDraw";
import type { SemanticOledState as DesktopSemanticOledState } from "../src/ui/OledDisplay";

const GREEN_CSS = "rgb(0, 255, 0)";

type RectCall = { x: number; y: number; width: number; height: number; fillStyle: string };
type TextCall = { text: string; x: number; y: number; fillStyle: string };

class FakeCanvasContext {
  fillStyle = "";
  font = "";
  textBaseline = "";
  readonly rects: RectCall[] = [];
  readonly texts: TextCall[] = [];

  clearRect(): void {}
  fillRect(x: number, y: number, width: number, height: number): void {
    this.rects.push({ x, y, width, height, fillStyle: this.fillStyle });
  }
  fillText(text: string, x: number, y: number): void {
    this.texts.push({ text, x, y, fillStyle: this.fillStyle });
  }
  beginPath(): void {}
  arc(): void {}
  fill(): void {}
  drawImage(): void {}
  moveTo(): void {}
  lineTo(): void {}
  closePath(): void {}
}

function semantic(overrides: Partial<DesktopSemanticOledState> = {}): DesktopSemanticOledState {
  return {
    displayOff: false,
    splashText: "",
    title: "B",
    lines: ["  Parameter Name With Extra Words"],
    selectedRow: -1,
    lineColors: [0x07e0],
    barValues: [{ frac: 0 }],
    scroll: null,
    transportIcon: "stop",
    eventDotOn: false,
    eventDotSteal: false,
    transportFlash: "none",
    visibleFooterToast: "",
    cpuLoad: 0,
    ...overrides,
  };
}

function barRects(ctx: FakeCanvasContext): RectCall[] {
  return ctx.rects.filter((rect) => rect.x >= 87 && rect.y >= 17 && rect.y <= 25);
}

test("desktop OLED bars render empty, partial, full, and marker states inside a bounded box", () => {
  const ctx = new FakeCanvasContext();
  drawSemanticOled(ctx as unknown as CanvasRenderingContext2D, semantic(), null, null);

  assert.deepEqual(barRects(ctx).slice(0, 2).map((rect) => [rect.x, rect.y, rect.width, rect.height]), [
    [87, 17, 36, 9],
    [88, 18, 34, 7],
  ]);
  assert.equal(ctx.texts.find((text) => text.x === 4 && text.y === 18)?.text, "  Parameter N");

  const partial = new FakeCanvasContext();
  drawSemanticOled(partial as unknown as CanvasRenderingContext2D, semantic({ barValues: [{ frac: 0.5 }] }), null, null);
  assert.ok(partial.rects.some((rect) => isRect(rect, 88, 18, 17, 7, GREEN_CSS)));

  const full = new FakeCanvasContext();
  drawSemanticOled(full as unknown as CanvasRenderingContext2D, semantic({ barValues: [{ frac: 1 }] }), null, null);
  assert.ok(full.rects.some((rect) => isRect(rect, 88, 18, 34, 7, GREEN_CSS)));

  const marker = new FakeCanvasContext();
  drawSemanticOled(
    marker as unknown as CanvasRenderingContext2D,
    semantic({ barValues: [{ frac: 0.5, style: "marker" }] }),
    null,
    null,
  );
  assert.ok(marker.rects.some((rect) => isRect(rect, 105, 19, 1, 5, GREEN_CSS)));
});

test("desktop OLED selected bars keep a dark cutout while preserving the menu color fill", () => {
  const ctx = new FakeCanvasContext();

  drawSemanticOled(ctx as unknown as CanvasRenderingContext2D, semantic({ selectedRow: 0, barValues: [{ frac: 1 }] }), null, null);

  assert.ok(ctx.rects.some((rect) => isRect(rect, 87, 17, 36, 9, cssBlack())));
  assert.ok(ctx.rects.some((rect) => isRect(rect, 88, 18, 34, 7, GREEN_CSS)));
  assert.equal(ctx.texts.find((text) => text.x === 4 && text.y === 18)?.fillStyle, cssBlack());
});

function isRect(
  rect: RectCall,
  x: number,
  y: number,
  width: number,
  height: number,
  fillStyle: string,
): boolean {
  return rect.x === x && rect.y === y && rect.width === width && rect.height === height && rect.fillStyle === fillStyle;
}

function cssBlack(): string {
  return `rgb(${BLACK_COLOR[0]}, ${BLACK_COLOR[1]}, ${BLACK_COLOR[2]})`;
}
