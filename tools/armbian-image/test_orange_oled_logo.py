#!/usr/bin/env python3
import importlib.util
from importlib.machinery import SourceFileLoader
from pathlib import Path
import sys
import types


REPOSITORY = Path(__file__).resolve().parents[2]
SCRIPT = REPOSITORY / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
fcntl_stub = types.ModuleType("fcntl")
setattr(fcntl_stub, "ioctl", lambda *args: None)
sys.modules.setdefault("fcntl", fcntl_stub)
SPEC = importlib.util.spec_from_loader("octessera_orange_oled_logo", SourceFileLoader("octessera_orange_oled_logo", str(SCRIPT)))
assert SPEC is not None
assert SPEC.loader is not None
LOGO = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LOGO)
setattr(LOGO, "MARK_SOURCE", str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg"))
setattr(LOGO, "WORDMARK_SOURCE", str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg"))


def pixel(payload, x, y):
    offset = (y * LOGO.WIDTH + x) * 2
    return int.from_bytes(payload[offset:offset + 2], "big")


assert LOGO.BOOT_SWEEP_FRAME_COUNT == 24
assert len(LOGO.BOOT_SWEEP_COLORS) == 4
assert [LOGO.BOOT_SWEEP_COLORS[index % 4] for index in range(4)] == [
    (0, 255, 255),
    (255, 255, 0),
    (0, 255, 0),
    (255, 0, 255),
]

canvas = LOGO.logo_canvas("boot")
assert any(value == 255 for row in canvas for value in row)
frames = [LOGO.render_canvas(canvas, frame) for frame in range(LOGO.BOOT_SWEEP_FRAME_COUNT)]
assert len(frames) == LOGO.BOOT_SWEEP_FRAME_COUNT
assert all(len(frame) == LOGO.WIDTH * LOGO.HEIGHT * 2 for frame in frames)

frame = 12
y = LOGO.HEIGHT // 2
left = LOGO.sweep_band_left(frame, y)
colored_x = int(left) + 1
preserved_x = colored_x + 1
test_canvas = [[0 for _ in range(LOGO.WIDTH)] for _ in range(LOGO.HEIGHT)]
test_canvas[LOGO.HEIGHT - 1 - colored_x][y] = 255
test_canvas[LOGO.HEIGHT - 1 - preserved_x][y] = 127
rendered = LOGO.render_canvas(test_canvas, frame)
assert pixel(rendered, colored_x, y) == LOGO.rgb565_color(LOGO.BOOT_SWEEP_COLORS[frame % 4])
assert pixel(rendered, preserved_x, y) == LOGO.rgb565(127)

top_x = next(x for x in range(LOGO.WIDTH) if LOGO.in_sweep_band(x, 0, frame))
bottom_x = next(x for x in range(LOGO.WIDTH) if LOGO.in_sweep_band(x, LOGO.HEIGHT - 1, frame))
assert top_x > bottom_x


class FakeOled:
    instances = []

    def __init__(self):
        self.frames = []
        self.close_args = []
        self.initialized = False
        self.__class__.instances.append(self)

    def initialize(self):
        self.initialized = True

    def frame(self, payload):
        self.frames.append(payload)

    def close(self, display_off=True):
        self.close_args.append(display_off)


sleep_calls = []
real_oled = LOGO.Oled
real_sleep = LOGO.time.sleep
try:
    setattr(LOGO, "Oled", FakeOled)
    setattr(LOGO.time, "sleep", lambda seconds: sleep_calls.append(seconds))
    LOGO.run("boot")
    LOGO.run("sleep")
finally:
    setattr(LOGO, "Oled", real_oled)
    setattr(LOGO.time, "sleep", real_sleep)

boot_oled, sleep_oled = FakeOled.instances
assert boot_oled.initialized and sleep_oled.initialized
assert len(boot_oled.frames) == LOGO.BOOT_SWEEP_FRAME_COUNT
assert sleep_calls == [LOGO.BOOT_SWEEP_FRAME_DELAY_SECONDS] * (LOGO.BOOT_SWEEP_FRAME_COUNT - 1)
assert boot_oled.close_args == [False]
assert len(sleep_oled.frames) == 1
assert sleep_oled.close_args == [False]

print("Orange OLED logo sweep validation passed")
