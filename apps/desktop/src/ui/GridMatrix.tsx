import { GRID_WIDTH, type RuntimeSnapshot } from "@cellsymphony/device-contracts";

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
        {frame.leds.cells.map((cell, index) => {
          const x = index % GRID_WIDTH;
          const y = Math.floor(index / GRID_WIDTH);
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
