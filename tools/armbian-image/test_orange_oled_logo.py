#!/usr/bin/env python3
import hashlib
import io
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
logo.BOOT_RGB565_SOURCE = str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565")
logo.SHUTDOWN_RGB565_SOURCE = str(REPOSITORY / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565")
contract = json.loads(CONTRACT.read_text(encoding="utf-8"))


def exact(value, keys):
    assert set(value) == set(keys)


exact(contract, ["schema_version", "strictness", "coordinate_space", "source_pixel_rule", "bands", "slant", "timing", "wire_budget", "travel", "golden_samples"])
assert contract["schema_version"] == 1
exact(contract["strictness"], ["unknown_keys", "missing_keys"])
assert contract["strictness"] == {"unknown_keys": "reject", "missing_keys": "reject"}
exact(contract["coordinate_space"], ["orientation", "width_px", "height_px", "x_direction", "y_direction", "physical_motion"])
assert contract["coordinate_space"]["x_direction"] == "leftward_controller_axis"
assert contract["coordinate_space"]["physical_motion"] == "left_to_right_after_mounted_ssd1351_remap"
exact(contract["source_pixel_rule"], ["pixel_format", "recolor_match_rgb565", "match_action", "non_match_action"])
exact(contract["bands"], ["order", "band_count", "band_width_px", "separator_width_px", "separator_color_rgb565", "separator_semantics", "train_width_px", "items"])
for item in contract["bands"]["items"]:
    exact(item, ["band_index", "name", "color_rgb565", "width_px"])
exact(contract["slant"], ["offset_formula", "offset_numerator_px", "offset_denominator_rows", "row_y_min", "row_y_max", "bottom_row_offset_px", "top_row_offset_px"])
assert contract["slant"]["offset_formula"] == "-row_y"
exact(contract["timing"], ["cycle_duration_ns", "frames_per_cycle", "frame_index_min", "frame_index_max", "frame_deadline_offset_formula", "frame_deadline_reference", "scheduling_mode", "cumulative_sleep_scheduling"])
exact(contract["wire_budget"], ["spi_clock_hz", "address_command_bytes_per_cycle", "frame_data_bytes", "conservative_command_data_bytes_per_frame", "utilization_limit_percent", "cycle_frame_count", "cycle_payload_duration_ns", "cycle_utilization_percent", "utilization_headroom_to_limit_percent", "accepted_frame_count", "rejected_frame_count"])
exact(contract["travel"], ["bottom_row_origin_formula", "frame_index_min", "frame_index_max", "start_bottom_row_origin_px", "end_bottom_row_origin_px", "travel_distance_px", "pixel_membership", "endpoint_blank_frames", "wrap"])
exact(contract["travel"]["pixel_membership"], ["slanted_origin_formula", "local_x_formula", "in_band_condition", "band_index_formula", "separator_condition", "separator_action", "outside_action", "inside_action"])
exact(contract["travel"]["endpoint_blank_frames"], ["frame_indices", "intentional", "extra_pause_inserted", "frame_0", "frame_29"])
exact(contract["travel"]["endpoint_blank_frames"]["frame_0"], ["bottom_row_origin_px", "top_row_origin_px", "leftmost_train_pixel_px", "fully_offscreen_right"])
exact(contract["travel"]["endpoint_blank_frames"]["frame_29"], ["bottom_row_origin_px", "top_row_origin_px", "rightmost_train_pixel_px", "fully_offscreen_left"])
exact(contract["travel"]["wrap"], ["after_frame_index", "next_frame_index", "extra_pause_inserted"])
exact(contract["golden_samples"], ["pixel_samples", "geometry_samples", "endpoint_assertions"])
pixel_identities = set()
for sample in contract["golden_samples"]["pixel_samples"]:
    exact(sample, ["sample_group", "frame_index", "x", "y", "source_rgb565", "expected_rgb565"])
    assert all(isinstance(sample[key], int) for key in ("frame_index", "x", "y"))
    assert all(isinstance(sample[key], str) and len(sample[key]) == 4 for key in ("source_rgb565", "expected_rgb565"))
    identity = (sample["frame_index"], sample["x"], sample["y"], sample["source_rgb565"])
    assert identity not in pixel_identities, f"duplicate pixel golden sample identity: {identity}"
    pixel_identities.add(identity)
geometry_identities = set()
for sample in contract["golden_samples"]["geometry_samples"]:
    geometry_keys = ["sample_group", "frame_index", "row_y", "expected_slant_offset_px", "expected_slanted_origin_px"]
    if sample["row_y"] == 0:
        geometry_keys.append("expected_bottom_row_origin_px")
    exact(sample, geometry_keys)
    identity = (sample["frame_index"], sample["row_y"])
    assert identity not in geometry_identities, f"duplicate geometry golden sample identity: {identity}"
    geometry_identities.add(identity)
endpoint = contract["golden_samples"]["endpoint_assertions"]
exact(endpoint, ["frame_0", "frame_29", "cycle"])
exact(endpoint["frame_0"], ["expected_bottom_row_origin_px", "expected_deadline_offset_ns", "expected_next_frame_index", "fully_offscreen_right"])
exact(endpoint["frame_29"], ["expected_bottom_row_origin_px", "expected_deadline_offset_ns", "expected_next_frame_index", "fully_offscreen_left"])
exact(endpoint["cycle"], ["expected_frame_count", "expected_first_frame_index", "expected_last_frame_index", "expected_wrap_frame_index", "extra_pause_inserted"])

assert hashlib.sha256(CONTRACT.read_bytes()).hexdigest() == logo.BOOT_SWEEP_CONTRACT_SHA256
assert logo.BOOT_SWEEP_FRAME_COUNT == contract["timing"]["frames_per_cycle"] == 30
assert logo.BOOT_SWEEP_DURATION_NS == contract["timing"]["cycle_duration_ns"]
assert logo.BOOT_SWEEP_REST_NS == 2_000_000_000
assert logo.BOOT_SWEEP_REST_CHECK_NS == 50_000_000
assert logo.BOOT_SWEEP_SEPARATOR_WIDTH == contract["bands"]["separator_width_px"] == 4
assert logo.BOOT_SWEEP_SEPARATOR_COLOR_RGB565 == int(contract["bands"]["separator_color_rgb565"], 16) == 0xFFFF
assert logo.BOOT_SWEEP_TRAIN_WIDTH == contract["bands"]["train_width_px"] == 48
assert logo.BOOT_SWEEP_COLORS_RGB565 == tuple(int(item["color_rgb565"], 16) for item in contract["bands"]["items"])
assert contract["wire_budget"]["spi_clock_hz"] == 16_000_000
assert contract["wire_budget"]["address_command_bytes_per_cycle"] == 7
assert contract["wire_budget"]["frame_data_bytes"] == logo.FRAME_BYTES == 32_768
assert contract["wire_budget"]["conservative_command_data_bytes_per_frame"] == 32_775
assert contract["wire_budget"]["utilization_limit_percent"] == 80
assert contract["wire_budget"]["cycle_frame_count"] == 30
assert contract["wire_budget"]["cycle_payload_duration_ns"] == 491_625_000
assert contract["wire_budget"]["cycle_utilization_percent"] == 40.96875
assert contract["wire_budget"]["utilization_headroom_to_limit_percent"] == 39.03125
assert contract["wire_budget"]["accepted_frame_count"] == 58
assert contract["wire_budget"]["rejected_frame_count"] == 59
def under_wire_limit(frame_count):
    return frame_count * 32_775 * 8 * 100 * 1_000_000_000 <= 16_000_000 * logo.BOOT_SWEEP_DURATION_NS * 80
assert under_wire_limit(58)
assert not under_wire_limit(59)
assert [logo.sweep_deadline_offset_ns(frame) for frame in range(30)] == [frame * 40_000_000 for frame in range(30)]
assert logo.sweep_band_left(15, 0) == 99
assert logo.sweep_band_left(15, 127) == -28
assert logo.sweep_band_left(15, 127) < logo.sweep_band_left(15, 0)
assert contract["travel"]["start_bottom_row_origin_px"] == 255
assert contract["travel"]["end_bottom_row_origin_px"] == -48
assert contract["travel"]["travel_distance_px"] == -303
for frame in range(logo.BOOT_SWEEP_FRAME_COUNT):
    assert logo.sweep_band_left(frame, 127) - logo.sweep_band_left(frame, 0) == -127
for first in range(logo.BOOT_SWEEP_FRAME_COUNT):
    for second in range(first, logo.BOOT_SWEEP_FRAME_COUNT):
        delta = logo.sweep_band_left(second, 0) - logo.sweep_band_left(first, 0)
        assert all(logo.sweep_band_left(second, row) - logo.sweep_band_left(first, row) == delta for row in (0, 64, 127))
assert logo.logo_canvas("boot") == bytearray((REPOSITORY / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-booting.rgb565").read_bytes())
assert logo.logo_canvas("shutdown") == bytearray((REPOSITORY / "userpatches/overlay/usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565").read_bytes())
assert logo.logo_canvas("shutdown") == logo.logo_canvas("boot")
assert [logo.parse_mode([mode]) for mode in ("boot-once", "boot-static", "boot-loop", "resume", "sleep", "shutdown", "off")] == ["boot-once", "boot-static", "boot-loop", "resume", "sleep", "shutdown", "off"]
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
    {"frame_index": 6, "x": 3, "y": 1, "source_rgb565": "FFFF", "expected_rgb565": "FFFF"},
    {"frame_index": 6, "x": 10, "y": 126, "source_rgb565": "FFFF", "expected_rgb565": "FFFF"},
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
    bottom_origin = logo.sweep_band_left(frame, 0)
    slant = row * logo.BOOT_SWEEP_LEAN_PIXELS // logo.BOOT_SWEEP_LEAN_DENOMINATOR
    assert sample["expected_slant_offset_px"] == slant
    if row == 0:
        assert bottom_origin == sample["expected_bottom_row_origin_px"]
    assert logo.sweep_band_left(frame, row) == sample["expected_slanted_origin_px"]
travel_endpoint = contract["travel"]["endpoint_blank_frames"]
for frame_name, frame_index in (("frame_0", 0), ("frame_29", logo.BOOT_SWEEP_FRAME_COUNT - 1)):
    bottom_origin = logo.sweep_band_left(frame_index, 0)
    top_origin = logo.sweep_band_left(frame_index, logo.HEIGHT - 1)
    endpoint = travel_endpoint[frame_name]
    assert endpoint["bottom_row_origin_px"] == bottom_origin
    assert endpoint["top_row_origin_px"] == top_origin
    if frame_index == 0:
        leftmost = top_origin
        assert endpoint["leftmost_train_pixel_px"] == leftmost
        assert endpoint["fully_offscreen_right"] == (leftmost >= logo.WIDTH)
    else:
        rightmost = bottom_origin + logo.BOOT_SWEEP_TRAIN_WIDTH - 1
        assert endpoint["rightmost_train_pixel_px"] == rightmost
        assert endpoint["fully_offscreen_left"] == (rightmost < 0)
endpoint = contract["golden_samples"]["endpoint_assertions"]
for frame_name, frame_index in (("frame_0", 0), ("frame_29", logo.BOOT_SWEEP_FRAME_COUNT - 1)):
    golden = endpoint[frame_name]
    assert golden["expected_bottom_row_origin_px"] == logo.sweep_band_left(frame_index, 0)
    assert golden["expected_deadline_offset_ns"] == logo.sweep_deadline_offset_ns(frame_index)
    assert golden["expected_next_frame_index"] == (frame_index + 1) % logo.BOOT_SWEEP_FRAME_COUNT
canvas = logo.logo_canvas("boot")
assert any(value == 255 for value in canvas)
frames = [logo.render_canvas(canvas, frame) for frame in range(logo.BOOT_SWEEP_FRAME_COUNT)]
assert all(len(frame) == logo.WIDTH * logo.HEIGHT * 2 for frame in frames)
raw = bytearray(b"\xFF" * logo.FRAME_BYTES)
first = logo.apply_sweep(logo.rotate_clockwise_rgb565(raw), 14)
second = logo.apply_sweep(logo.rotate_clockwise_rgb565(raw), 15)
for local_x in range(4, 44):
    first_x = logo.sweep_band_left(14, 64) + local_x
    second_x = logo.sweep_band_left(15, 64) + local_x
    assert first[((logo.HEIGHT - 1 - 64) * logo.WIDTH + first_x) * 2:][:2] == second[((logo.HEIGHT - 1 - 64) * logo.WIDTH + second_x) * 2:][:2]

class RecordingGpio:
    def __init__(self, events):
        self.events = events

    def set(self, name, offset, value):
        assert name == "dc"
        assert offset == logo.GPIO_DC
        self.events.append(("dc", value))


def recording_oled(events):
    oled = logo.Oled.__new__(logo.Oled)
    oled.gpio = RecordingGpio(events)
    oled.write = lambda payload: events.append(("write", bytes(payload)))
    return oled


command_oled_events: list[tuple[str, object]] = []
command_oled = recording_oled(command_oled_events)
command_oled.command(0xAE)
assert command_oled_events == [("dc", 0), ("write", b"\xae")]
command_oled_events.clear()
command_oled.command(0xA0, 0x74)
assert command_oled_events == [("dc", 0), ("write", b"\xa0"), ("dc", 1), ("write", b"\x74")]

frame_oled_events: list[tuple[str, object]] = []
frame_oled = recording_oled(frame_oled_events)
frame_oled.frame(bytes(logo.FRAME_BYTES))
assert frame_oled_events == [
    ("dc", 0), ("write", b"\x15"), ("dc", 1), ("write", b"\x00\x7f"),
    ("dc", 0), ("write", b"\x75"), ("dc", 1), ("write", b"\x00\x7f"),
    ("dc", 0), ("write", b"\x5c"), ("dc", 1), ("write", bytes(logo.FRAME_BYTES)),
]
stream_oled_events: list[tuple[str, object]] = []
stream_oled = recording_oled(stream_oled_events)
stream_oled.begin_frame_stream()
stream_oled_events.clear()
stream_oled.stream_frame(bytes(logo.FRAME_BYTES))
stream_oled.stream_frame(bytes(logo.FRAME_BYTES))
assert stream_oled_events == [("write", bytes(logo.FRAME_BYTES)), ("write", bytes(logo.FRAME_BYTES))]
try:
    stream_oled.stream_frame(bytes(logo.FRAME_BYTES - 1))
except ValueError as error:
    assert str(error) == f"OLED stream frame must contain exactly {logo.FRAME_BYTES} bytes"
else:
    raise AssertionError("short OLED stream frame was accepted")
initialization_events: list[tuple[str, object]] = []
initialization_oled = recording_oled(initialization_events)
initialization_oled.reset = lambda: initialization_events.append(("reset", None))
initialization_oled.initialize()
initialization_commands = (
    (0xFD, 0x12),
    (0xFD, 0xB1),
    (0xAE,),
    (0xB3, 0xF1),
    (0xCA, 0x7F),
    (0xA0, 0x74),
    (0x15, 0x00, 0x7F),
    (0x75, 0x00, 0x7F),
    (0xA1, 0x00),
    (0xA2, 0x00),
    (0xB5, 0x00),
    (0xAB, 0x01),
    (0xB1, 0x32),
    (0xBB, 0x17),
    (0xBE, 0x05),
    (0xA6,),
    (0xC1, 0xC8, 0x80, 0xC8),
    (0xC7, 0x0F),
    (0xB4, 0xA0, 0xB5, 0x55),
    (0xB6, 0x01),
    (0xAF,),
)
expected_initialization_events: list[tuple[str, object]] = [("reset", None)]
for values in initialization_commands:
    expected_initialization_events.extend((("dc", 0), ("write", bytes((values[0],)))))
    if len(values) > 1:
        expected_initialization_events.extend((("dc", 1), ("write", bytes(values[1:]))))
assert initialization_events == expected_initialization_events


class FakeOled:
    instances = []

    def __init__(self):
        self.frames = []
        self.begin_frame_stream_calls = 0
        self.close_args = []
        self.initialized = False
        self.__class__.instances.append(self)

    def initialize(self):
        self.initialized = True

    def frame(self, payload):
        self.begin_frame_stream()
        self.stream_frame(payload)

    def begin_frame_stream(self):
        self.begin_frame_stream_calls += 1

    def stream_frame(self, payload):
        self.frames.append(payload)

    def close(self, display_off=True):
        self.close_args.append(display_off)


class FakeLoopHandoff:
    def __init__(self, phase):
        self.boot_id = "01234567-89ab-cdef-0123-456789abcdef"
        self.status = {"phase": phase, "bootId": self.boot_id}
        self.start_calls = 0
        self.mark_failed_calls = 0
        self.close_calls = 0

    def _read_status(self):
        return self.status

    def start(self):
        self.start_calls += 1
        raise RuntimeError("OLED handoff already exists for this boot")

    def mark_failed(self):
        self.mark_failed_calls += 1

    def close(self):
        self.close_calls += 1


def unexpected_oled():
    raise AssertionError("native-owned OLED handoff created an OLED")


real_loop_handoff = logo.Handoff
real_loop_oled = logo.Oled
try:
    for phase in ("released", "native_owned", "first_menu_rendered"):
        loop_handoff = FakeLoopHandoff(phase)
        logo.Handoff = types.SimpleNamespace(open=lambda create_lock, handoff=loop_handoff: handoff)
        logo.Oled = unexpected_oled
        logo._run_loop()
        assert loop_handoff.start_calls == 0
        assert loop_handoff.mark_failed_calls == 0
        assert loop_handoff.close_calls == 1

    loop_handoff = FakeLoopHandoff("animating")
    logo.Handoff = types.SimpleNamespace(open=lambda create_lock, handoff=loop_handoff: handoff)
    logo.Oled = unexpected_oled
    try:
        logo._run_loop()
    except RuntimeError as error:
        assert str(error) == "OLED handoff already exists for this boot"
    else:
        raise AssertionError("non-native same-boot OLED handoff was accepted")
    assert loop_handoff.start_calls == 1
    assert loop_handoff.mark_failed_calls == 1
    assert loop_handoff.close_calls == 1
finally:
    logo.Handoff = real_loop_handoff
    logo.Oled = real_loop_oled


class FakeGpioProcess:
    def __init__(self, command, **kwargs):
        self.command = command
        self.kwargs = kwargs
        self.stderr = io.StringIO()
        self.terminated = False
        self.killed = False
        self.exit_code = None

    def poll(self):
        return self.exit_code

    def terminate(self):
        self.terminated = True
        self.exit_code = 0

    def kill(self):
        self.killed = True
        self.exit_code = -9

    def wait(self, timeout=None):
        return self.exit_code


gpio_processes = []
real_stat = logo.os.stat
real_is_char = logo.stat.S_ISCHR
real_popen = logo.subprocess.Popen
real_gpio_sleep = logo.time.sleep
try:
    logo.os.stat = lambda path: types.SimpleNamespace(st_mode=0)
    logo.stat.S_ISCHR = lambda mode: True
    logo.subprocess.Popen = lambda command, **kwargs: gpio_processes.append(FakeGpioProcess(command, **kwargs)) or gpio_processes[-1]
    logo.time.sleep = lambda seconds: None
    gpio = logo.GpioLines()
    gpio.set("dc", logo.GPIO_DC, 0)
    first = gpio_processes[-1]
    assert first.command == ["gpioset", "--chip", logo.GPIO_CHIP, f"{logo.GPIO_DC}=0"]
    assert first.kwargs == {"stdout": logo.subprocess.DEVNULL, "stderr": logo.subprocess.PIPE, "text": True}
    assert gpio.processes["dc"] == (0, first)
    process_count = len(gpio_processes)
    gpio.set("dc", logo.GPIO_DC, 0)
    assert len(gpio_processes) == process_count
    assert gpio.processes["dc"] == (0, first)
    gpio.set("dc", logo.GPIO_DC, 1)
    second = gpio_processes[-1]
    assert first.terminated and not first.killed
    assert second.command == ["gpioset", "--chip", logo.GPIO_CHIP, f"{logo.GPIO_DC}=1"]
    gpio.close()
    assert second.terminated and not second.killed

    dead_gpio = logo.GpioLines()
    dead_gpio.set("dc", logo.GPIO_DC, 0)
    dead = gpio_processes[-1]
    dead.exit_code = 1
    dead_process_count = len(gpio_processes)
    try:
        dead_gpio.set("dc", logo.GPIO_DC, 0)
    except RuntimeError as error:
        assert str(error) == "H618 GPIO dc holder exited unexpectedly"
    else:
        raise AssertionError("dead GPIO holder was restarted for the same value")
    try:
        dead_gpio.set("dc", logo.GPIO_DC, 1)
    except RuntimeError as error:
        assert str(error) == "H618 GPIO dc holder exited unexpectedly"
    else:
        raise AssertionError("dead GPIO holder was replaced for a different value")
    assert len(gpio_processes) == dead_process_count
    assert dead_gpio.processes["dc"] == (0, dead)
    dead_gpio.close()

    def failed_popen(command, **kwargs):
        process = FakeGpioProcess(command, **kwargs)
        process.stderr = io.StringIO("gpioset: invalid line\n")
        process.exit_code = 1
        return process

    logo.subprocess.Popen = failed_popen
    try:
        logo.GpioLines().set("reset", logo.GPIO_RESET, 1)
    except RuntimeError as error:
        assert str(error) == f"could not set H618 GPIO {logo.GPIO_RESET}: gpioset: invalid line"
    else:
        raise AssertionError("failed gpioset startup was accepted")
finally:
    logo.os.stat = real_stat
    logo.stat.S_ISCHR = real_is_char
    logo.subprocess.Popen = real_popen
    logo.time.sleep = real_gpio_sleep


print("Orange OLED contract, static mode, golden sweep, and visual cleanup tests passed")
