import { type RuntimeSnapshot } from '@octessera/device-contracts';
import { gridLedColor } from './gridLedColor';

export function GridMatrix({
  frame,
  onTileDrag,
  onTileMouseDown,
}: {
  frame: RuntimeSnapshot;
  onTileDrag: (x: number, y: number) => void;
  onTileMouseDown: (index: number, x: number, y: number) => void;
}) {
  return (
    <section className="matrix-chassis" aria-label="8 by 8 matrix">
      <div className="matrix">
        {Array.from(
          { length: frame.leds.width * frame.leds.height },
          (_, index) => {
            const tile = gridLedColor(frame, index);
            const x = index % frame.leds.width;
            const y = Math.floor(index / frame.leds.width);
            return (
              <button
                key={`${x}-${y}`}
                type="button"
                aria-label={`Grid ${x},${y}`}
                className="tile"
                style={{
                  backgroundColor: `rgb(${tile.r}, ${tile.g}, ${tile.b})`,
                }}
                onMouseDown={() => onTileMouseDown(index, x, y)}
                onMouseEnter={(event) => {
                  if (event.buttons !== 1) return;
                  onTileDrag(x, y);
                }}
                onClick={(event) => event.preventDefault()}
              />
            );
          },
        )}
      </div>
    </section>
  );
}
