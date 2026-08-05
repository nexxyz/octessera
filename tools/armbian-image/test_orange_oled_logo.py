#!/usr/bin/env python3
import hashlib
import importlib.util
import json
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path
import types


REPOSITORY = Path(__file__).resolve().parents[2]
SCRIPT = REPOSITORY / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
HANDOFF = REPOSITORY / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
CONTRACT = REPOSITORY / "resources/oled/boot-sweep-v1.json"
sys.path.insert(0, str(SCRIPT.parent))
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    pwd_stub.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    sys.modules["pwd"] = pwd_stub
fcntl_stub = types.ModuleType("fcntl")
setattr(fcntl_stub, "ioctl", lambda *args: None)
sys.modules.setdefault("fcntl", fcntl_stub)
handoff_spec = importlib.util.spec_from_loader("octessera_orange_oled_handoff", SourceFileLoader("octessera_orange_oled_handoff", str(HANDOFF)))
assert handoff_spec is not None and handoff_spec.loader is not None
handoff = importlib.util.module_from_spec(handoff_spec)
handoff_spec.loader.exec_module(handoff)
sys.modules["octessera_orange_oled_handoff"] = handoff
spec = importlib.util.spec_from_loader("octessera_orange_oled_logo", SourceFileLoader("octessera_orange_oled_logo", str(SCRIPT)))
assert spec is not None and spec.loader is not None
logo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logo)
logo.MARK_SOURCE = str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-mark.svg")
logo.WORDMARK_SOURCE = str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera-setup-ui/octessera-wordmark.svg")
contract = json.loads(CONTRACT.read_text(encoding="utf-8"))


def exact(value, keys):
    assert set(value) == set(keys)


exact(contract, ["schema_version", "strictness", "coordinate_space", "source_pixel_rule", "bands", "slant", "timing", "travel", "golden_samples"])
assert contract["schema_version"] == 1
exact(contract["strictness"], ["unknown_keys", "missing_keys"])
assert contract["strictness"] == {"unknown_keys": "reject", "missing_keys": "reject"}
exact(contract["coordinate_space"], ["orientation", "width_px", "height_px", "x_direction", "y_direction"])
exact(contract["source_pixel_rule"], ["pixel_format", "recolor_match_rgb565", "match_action", "non_match_action"])
exact(contract["bands"], ["order", "band_count", "band_width_px", "train_width_px", "items"])
for item in contract["bands"]["items"]:
    exact(item, ["band_index", "name", "color_rgb565", "width_px"])
exact(contract["slant"], ["offset_formula", "offset_numerator_px", "offset_denominator_rows", "row_y_min", "row_y_max", "bottom_row_offset_px", "top_row_offset_px"])
exact(contract["timing"], ["cycle_duration_ns", "frames_per_cycle", "frame_index_min", "frame_index_max", "frame_deadline_offset_formula", "frame_deadline_reference", "scheduling_mode", "cumulative_sleep_scheduling"])
exact(contract["travel"], ["bottom_row_origin_formula", "frame_index_min", "frame_index_max", "start_bottom_row_origin_px", "end_bottom_row_origin_px", "travel_distance_px", "pixel_membership", "endpoint_blank_frames", "wrap"])
exact(contract["travel"]["pixel_membership"], ["slanted_origin_formula", "local_x_formula", "in_band_condition", "band_index_formula", "outside_action", "inside_action"])
exact(contract["travel"]["endpoint_blank_frames"], ["frame_indices", "intentional", "extra_pause_inserted", "frame_0", "frame_23"])
exact(contract["travel"]["endpoint_blank_frames"]["frame_0"], ["bottom_row_origin_px", "top_row_origin_px", "rightmost_train_pixel_px", "fully_offscreen_left"])
exact(contract["travel"]["endpoint_blank_frames"]["frame_23"], ["bottom_row_origin_px", "top_row_origin_px", "leftmost_train_pixel_px", "fully_offscreen_right"])
exact(contract["travel"]["wrap"], ["after_frame_index", "next_frame_index", "extra_pause_inserted"])
exact(contract["golden_samples"], ["pixel_samples", "geometry_samples", "endpoint_assertions"])
for sample in contract["golden_samples"]["pixel_samples"]:
    exact(sample, ["sample_group", "frame_index", "x", "y", "source_rgb565", "expected_rgb565"])
    assert all(isinstance(sample[key], int) for key in ("frame_index", "x", "y"))
    assert all(isinstance(sample[key], str) and len(sample[key]) == 4 for key in ("source_rgb565", "expected_rgb565"))
for sample in contract["golden_samples"]["geometry_samples"]:
    geometry_keys = ["sample_group", "frame_index", "row_y", "expected_slant_offset_px", "expected_slanted_origin_px"]
    if sample["row_y"] == 0:
        geometry_keys.append("expected_bottom_row_origin_px")
    exact(sample, geometry_keys)
endpoint = contract["golden_samples"]["endpoint_assertions"]
exact(endpoint, ["frame_0", "frame_23", "cycle"])
exact(endpoint["frame_0"], ["expected_bottom_row_origin_px", "expected_deadline_offset_ns", "expected_next_frame_index", "fully_offscreen_left"])
exact(endpoint["frame_23"], ["expected_bottom_row_origin_px", "expected_deadline_offset_ns", "expected_next_frame_index", "fully_offscreen_right"])
exact(endpoint["cycle"], ["expected_frame_count", "expected_first_frame_index", "expected_last_frame_index", "expected_wrap_frame_index", "extra_pause_inserted"])

assert hashlib.sha256(CONTRACT.read_bytes()).hexdigest() == logo.BOOT_SWEEP_CONTRACT_SHA256
assert logo.BOOT_SWEEP_FRAME_COUNT == contract["timing"]["frames_per_cycle"] == 24
assert logo.BOOT_SWEEP_DURATION_NS == contract["timing"]["cycle_duration_ns"]
assert logo.BOOT_SWEEP_COLORS_RGB565 == tuple(int(item["color_rgb565"], 16) for item in contract["bands"]["items"])
assert [logo.parse_mode([mode]) for mode in ("boot-once", "boot-loop", "resume", "sleep", "shutdown")] == ["boot-once", "boot-loop", "resume", "sleep", "shutdown"]
for invalid in ([], ["boot"], ["boot-once", "sleep"], ["unknown"]):
    try:
        logo.parse_mode(invalid)
    except ValueError:
        pass
    else:
        raise AssertionError(f"invalid OLED mode accepted: {invalid}")

for sample in contract["golden_samples"]["pixel_samples"]:
    raw = bytearray(logo.WIDTH * logo.HEIGHT * 2)
    source_x = logo.HEIGHT - 1 - sample["y"]
    source_y = logo.WIDTH - 1 - sample["x"]
    offset = (source_y * logo.WIDTH + source_x) * 2
    raw[offset:offset + 2] = bytes.fromhex(sample["source_rgb565"])
    rendered = logo.apply_sweep(logo.rotate_clockwise_rgb565(raw), sample["frame_index"])
    destination = ((logo.HEIGHT - 1 - sample["y"]) * logo.WIDTH + sample["x"]) * 2
    assert rendered[destination:destination + 2] == bytes.fromhex(sample["expected_rgb565"]), sample

for sample in (
    {"frame_index": 6, "x": 3, "y": 1, "source_rgb565": "FFFF", "expected_rgb565": "07FF"},
    {"frame_index": 6, "x": 10, "y": 126, "source_rgb565": "FFFF", "expected_rgb565": "07FF"},
):
    raw = bytearray(logo.WIDTH * logo.HEIGHT * 2)
    source_x = logo.HEIGHT - 1 - sample["y"]
    source_y = logo.WIDTH - 1 - sample["x"]
    offset = (source_y * logo.WIDTH + source_x) * 2
    raw[offset:offset + 2] = bytes.fromhex(sample["source_rgb565"])
    rendered = logo.apply_sweep(logo.rotate_clockwise_rgb565(raw), sample["frame_index"])
    destination = ((logo.HEIGHT - 1 - sample["y"]) * logo.WIDTH + sample["x"]) * 2
    assert rendered[destination:destination + 2] == bytes.fromhex(sample["expected_rgb565"]), sample

for sample in contract["golden_samples"]["geometry_samples"]:
    frame = sample["frame_index"]
    row = sample["row_y"]
    if row == 0:
        assert -40 + (frame * 168) // 23 == sample["expected_bottom_row_origin_px"]
    assert (row * 8) // 127 == sample["expected_slant_offset_px"]
    assert logo.sweep_band_left(frame, row) == sample["expected_slanted_origin_px"]
assert logo.sweep_band_left(0, 127) + 31 == -1
assert logo.sweep_band_left(23, 0) == 128
assert logo.sweep_deadline_offset_ns(0) == 0
assert logo.sweep_deadline_offset_ns(23) == 958333333

canvas = logo.logo_canvas("boot")
assert any(value == 255 for row in canvas for value in row)
frames = [logo.render_canvas(canvas, frame) for frame in range(logo.BOOT_SWEEP_FRAME_COUNT)]
assert all(len(frame) == logo.WIDTH * logo.HEIGHT * 2 for frame in frames)


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


class FakeUtilityLock:
    def close(self):
        pass


sleep_calls = []
real_oled = logo.Oled
real_sleep = logo.time.sleep
real_handoff = logo.Handoff
real_drop_to_runtime = logo.drop_to_runtime
try:
    logo.Oled = FakeOled
    logo.Handoff = types.SimpleNamespace(utility_lock=lambda timeout: FakeUtilityLock())
    logo.drop_to_runtime = lambda: None
    logo.time.sleep = lambda seconds: sleep_calls.append(seconds)
    logo.run("boot-once")
    logo.run("sleep")
    logo.run("shutdown")
finally:
    logo.Oled = real_oled
    logo.time.sleep = real_sleep
    logo.Handoff = real_handoff
    logo.drop_to_runtime = real_drop_to_runtime

boot_oled, sleep_oled, shutdown_oled = FakeOled.instances
assert boot_oled.initialized and len(boot_oled.frames) == 24 and boot_oled.close_args == [False]
assert sleep_oled.initialized and len(sleep_oled.frames) == 1 and sleep_oled.close_args == [False]
assert shutdown_oled.initialized and len(shutdown_oled.frames) == 1 and shutdown_oled.close_args == [False]
assert len(sleep_calls) == 23


def busy_lock(timeout_seconds):
    raise TimeoutError("test OLED lock contention")


before_busy = len(FakeOled.instances)
logo.Handoff = types.SimpleNamespace(utility_lock=busy_lock)
logo.drop_to_runtime = lambda: None
logo.run("sleep")
logo.run("resume")
logo.run("shutdown")
assert len(FakeOled.instances) == before_busy
logo.Handoff = real_handoff
logo.drop_to_runtime = real_drop_to_runtime

print("Orange OLED contract, modes, golden sweep, and visual cleanup tests passed")
