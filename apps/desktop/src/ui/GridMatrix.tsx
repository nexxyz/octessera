import { type RuntimeSnapshot } from "@octessera/device-contracts";
import { gridLedColor } from "./gridLedColor";

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
          const cell = gridLedColor(frame, index);
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
