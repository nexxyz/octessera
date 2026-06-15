import { useEffect, useMemo, useRef, useState } from "react";
import { OLED_HEIGHT, OLED_WIDTH, type OledFrame, type RuntimeSnapshot } from "@cellsymphony/device-contracts";
import { saveFlashVisible } from "./saveFlash";

export function OledDisplay({ frame, displayBrightness }: { frame: RuntimeSnapshot; displayBrightness: number }) {
  const oledImage = useMemo(() => toOledImage(frame.oled), [frame.oled]);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  useOledCanvas(canvasRef, oledImage);

  return (
    <section className="oled-wrap">
      <div className="oled-bezel">
        <div className="oled-panel" style={{ width: OLED_WIDTH, height: OLED_HEIGHT, opacity: Math.max(0.2, displayBrightness / 100) }}>
          <canvas ref={canvasRef} className="oled-canvas" />
          {!oledImage ? <OledTextFallback frame={frame} /> : null}
        </div>
      </div>
    </section>
  );
}

function OledTextFallback({ frame }: { frame: RuntimeSnapshot }) {
  const selectedRow = Number((frame as any).selectedRow ?? -1);
  const lineColors = Array.isArray(frame.display.colors) ? frame.display.colors : [];
  const barValues = Array.isArray((frame.display as any).barValues) ? (frame.display as any).barValues : [];
  const transportIcon = String((frame as any).transportIcon ?? (frame.transport.playing ? "play" : "pause"));
  const eventDotOn = Boolean((frame as any).eventDotOn ?? false);
  const transportFlash = String((frame as any).transportFlash ?? "none");
  const autoSaveFlash = String(frame.settings?.autoSaveFlash ?? "none");
  const autoSaveFlashSerial = Number((frame.settings as any)?.autoSaveFlashSerial ?? 0);
  const [saveFlashStartedAt, setSaveFlashStartedAt] = useState<number | null>(autoSaveFlash === "flash" ? Date.now() : null);
  const [nowMs, setNowMs] = useState(() => Date.now());
  const cpuLoad = Number((frame as any).cpuLoadRatio ?? 0);
  const transportColor = transportFlash === "measure" ? "#ff3333" : transportFlash === "beat" ? "#33ff66" : "#d7ffe8";

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

  const showSaveFlash = saveFlashVisible(saveFlashStartedAt, nowMs);

  return (
    <div
      style={{
        position: "absolute",
        inset: 0,
        display: "flex",
        flexDirection: "column",
        justifyContent: "flex-start",
        padding: 5,
        boxSizing: "border-box",
        color: "#d7ffe8",
        fontFamily: "ui-monospace, SFMono-Regular, Consolas, monospace",
        fontSize: 9,
        lineHeight: 1.12,
        letterSpacing: 0.25,
        background: "radial-gradient(circle at top, rgba(26,65,52,0.35), rgba(0,0,0,0.85))"
      }}
    >
      <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 2, color: "#ffffff", minHeight: 10, gap: 6 }}>
        <span style={{ minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{frame.display.title}</span>
        <span style={{ display: "flex", gap: 4, alignItems: "center" }}>
          <span style={{ color: showSaveFlash ? "#fff3b0" : "#334433" }}>S</span>
          <span style={{ color: cpuLoad >= 0.85 ? "#ff6666" : cpuLoad >= 0.6 ? "#ffd166" : "#335544" }}>C</span>
        </span>
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 1, flex: 1, minHeight: 0, overflow: "hidden" }}>
        {frame.display.lines.map((line, index) => {
          const color = rgb565ToCss(typeof lineColors[index] === "number" ? lineColors[index] : 0xffff);
          const selected = index === selectedRow;
          const bar = barValues[index] && typeof barValues[index] === "object" ? (barValues[index] as { frac?: number; style?: string }) : null;
          const markerPct = Math.max(0, Math.min(100, Number(bar?.frac ?? 0) * 100));
          const barBackground = bar
            ? bar.style === "marker"
              ? `linear-gradient(90deg, transparent ${Math.max(0, markerPct - 1)}%, rgba(215,255,232,0.72) ${Math.max(0, markerPct - 1)}%, rgba(215,255,232,0.72) ${Math.min(100, markerPct + 1)}%, transparent ${Math.min(100, markerPct + 1)}%)`
              : `linear-gradient(90deg, rgba(215,255,232,0.28) ${markerPct}%, transparent ${markerPct}%)`
            : undefined;
          return (
            <div
              key={`oled-line-${index}`}
              style={{
                background: selected ? color : barBackground ?? "transparent",
                color: selected ? "#04120d" : color,
                padding: selected && line.startsWith("  ") ? "1px 5px 2px 3px" : "1px 3px",
                margin: "0 -2px",
                minHeight: 10,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap"
              }}
            >
              {line || " "}
            </div>
          );
        })}
      </div>
      <div style={{ marginTop: "auto", display: "flex", justifyContent: "flex-end", alignItems: "center", gap: 5, minHeight: 10 }}>
        <span style={{ color: transportColor }}>{transportIcon === "play" ? "▶" : transportIcon === "stop" ? "■" : "❚❚"}</span>
        <span style={{ color: eventDotOn ? "#ffffff" : "#334433" }}>●</span>
      </div>
    </div>
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

function useOledCanvas(ref: { current: HTMLCanvasElement | null }, image: ImageData | null): void {
  useEffect(() => {
    if (!image) return;
    const canvas = ref.current;
    if (!canvas) return;
    canvas.width = image.width;
    canvas.height = image.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.putImageData(image, 0, 0);
  }, [image, ref]);
}
