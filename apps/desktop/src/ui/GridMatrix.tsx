import { type RuntimeSnapshot } from "@cellsymphony/device-contracts";

const ledCell = (frame: RuntimeSnapshot, index: number) => {
  const offset = index * 3;
  const factor = frame.settings?.ledsDimmed ? 0.08 : 1;
  return {
    r: Math.round((frame.leds.rgb[offset] ?? 0) * factor),
    g: Math.round((frame.leds.rgb[offset + 1] ?? 0) * factor),
    b: Math.round((frame.leds.rgb[offset + 2] ?? 0) * factor)
  };
};

export function GridMatrix({
  frame,
  onCellDrag,
  onCellMouseDown
}: {
  frame: RuntimeSnapshot;
  onCellDrag: (x: number, y: number) => void;
  onCellMouseDown: (index: number, x: number, y: number) => void;
}) {
  return (
    <section className="matrix-chassis" aria-label="8 by 8 matrix">
      <div className="matrix">
        {Array.from({ length: frame.leds.width * frame.leds.height }, (_, index) => {
          const cell = ledCell(frame, index);
          const x = index % frame.leds.width;
          const y = Math.floor(index / frame.leds.width);
          return (
            <button
              key={`${x}-${y}`}
              type="button"
              aria-label={`Grid ${x},${y}`}
              className="cell"
              style={{ backgroundColor: `rgb(${cell.r}, ${cell.g}, ${cell.b})` }}
              onMouseDown={() => onCellMouseDown(index, x, y)}
              onMouseEnter={(event) => {
                if (event.buttons !== 1) return;
                onCellDrag(x, y);
              }}
              onClick={(event) => event.preventDefault()}
            />
          );
        })}
      </div>
    </section>
  );
}
