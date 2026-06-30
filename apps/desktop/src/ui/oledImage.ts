import type { OledFrame } from "@cellsymphony/device-contracts";

export function rgb565ToCss(value: number): string {
  const r5 = (value >> 11) & 0x1f;
  const g6 = (value >> 5) & 0x3f;
  const b5 = value & 0x1f;
  const r = (r5 << 3) | (r5 >> 2);
  const g = (g6 << 2) | (g6 >> 4);
  const b = (b5 << 3) | (b5 >> 2);
  return `rgb(${r}, ${g}, ${b})`;
}

export function toOledImage(oledFrame: OledFrame | undefined): ImageData | null {
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
