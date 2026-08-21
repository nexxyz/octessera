#!/usr/bin/env python3
import importlib.util
import signal
import sys
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-logo"
HANDOFF = ROOT / "userpatches/overlay/usr/local/sbin/octessera-orange-oled-handoff.py"
sys.path.insert(0, str(SCRIPT.parent))
try:
    import pwd  # noqa: F401
except ImportError:
    pwd_stub = types.ModuleType("pwd")
    pwd_stub.getpwnam = lambda name: (_ for _ in ()).throw(KeyError(name))
    sys.modules["pwd"] = pwd_stub
try:
    import fcntl
except ImportError:
    fcntl = types.ModuleType("fcntl")
    fcntl.ioctl = lambda *args: None
    sys.modules["fcntl"] = fcntl
handoff_spec = importlib.util.spec_from_loader("orange_oled_lifecycle_handoff", SourceFileLoader("orange_oled_lifecycle_handoff", str(HANDOFF)))
handoff = importlib.util.module_from_spec(handoff_spec)
handoff_spec.loader.exec_module(handoff)
sys.modules["octessera_orange_oled_handoff"] = handoff
spec = importlib.util.spec_from_loader("orange_oled_lifecycle_logo", SourceFileLoader("orange_oled_lifecycle_logo", str(SCRIPT)))
logo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(logo)


assert logo.NATIVE_HANDOFF_TIMEOUT_SECONDS == 30
assert logo.BOOT_SWEEP_FRAME_COUNT == 30
assert [logo.sweep_deadline_offset_ns(frame) for frame in range(30)] == [frame * 40_000_000 for frame in range(30)]


class FakeClock:
    def __init__(self, signal_module=None, signal_number=signal.SIGTERM):
        self.now = 0
        self.sleeps = []
        self.signal_module = signal_module
        self.signal_number = signal_number

    def monotonic_ns(self):
        return self.now

    def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.now += int(seconds * 1_000_000_000)
        if self.signal_module is not None and self.signal_module.trigger_on_sleep:
            self.signal_module.trigger_on_sleep = False
            self.signal_module.trigger(self.signal_number)


class FakeSignal:
    SIGTERM = signal.SIGTERM
    SIGINT = signal.SIGINT

    def __init__(self):
        self.handlers = {}
        self.trigger_on_sleep = False
        self.trigger_before_start = False

    def getsignal(self, signum):
        return self.handlers.get(signum, "default")

    def signal(self, signum, handler):
        self.handlers[signum] = handler
        if signum == self.SIGTERM and self.trigger_before_start:
            self.trigger_before_start = False
            handler(signum, None)

    def trigger(self, signum):
        self.handlers[signum](signum, None)


class LifecycleHandoff:
    def __init__(self, stop_at=None, start_error=None, initial_status=None, events=None, release_error=None):
        self.boot_id = "01234567-89ab-cdef-0123-456789abcdef"
        self.stop_at = stop_at
        self.clock = None
        self.status = initial_status
        self.start_error = start_error
        self.events = events if events is not None else []
        self.release_error = release_error
        self.mark_failed_calls = 0
        self.release_calls = 0
        self.close_calls = 0

    def _read_status(self):
        return self.status

    def start(self):
        self.events.append(("start",))
        if self.start_error is not None:
            raise self.start_error
        self.status = {"phase": "animating", "bootId": self.boot_id}

    def stop_requested(self):
        if self.stop_at is None or self.clock.now < self.stop_at:
            return False
        self.status = {"phase": "release_requested", "bootId": self.boot_id, "requestId": "0123456789abcdef0123456789abcdef"}
        return True

    def publish_cycle(self):
        self.events.append(("publish-cycle",))

    def release(self):
        self.events.append(("release",))
        self.release_calls += 1
        if self.release_error is not None:
            raise self.release_error

    def mark_failed(self):
        self.mark_failed_calls += 1
        self.status = {"phase": "failed", "bootId": self.boot_id, "requestId": "0123456789abcdef0123456789abcdef"}

    def close(self):
        self.events.append(("handoff-close",))
        self.close_calls += 1


class LifecycleOled:
    def __init__(self, events, fail_render=False, fail_black=False, fail_off=False, fail_close=False):
        self.events = events
        self.fail_render = fail_render
        self.fail_black = fail_black
        self.fail_off = fail_off
        self.fail_close = fail_close

    def initialize(self):
        self.events.append(("initialize",))

    def frame(self, payload):
        self.stream_frame(payload)

    def begin_frame_stream(self):
        self.events.append(("begin-frame-stream",))

    def stream_frame(self, payload):
        self.events.append(("frame", payload))
        if payload == bytes(logo.WIDTH * logo.HEIGHT * 2) and self.fail_black:
            raise RuntimeError("black failed")
        if payload == b"render" and self.fail_render:
            raise RuntimeError("render failed")

    def command(self, value):
        self.events.append(("command", value))
        if value == 0xAE and self.fail_off:
            raise RuntimeError("off failed")

    def close(self, display_off=True):
        self.events.append(("close", display_off))
        if not display_off and self.fail_close:
            raise RuntimeError("close failed")


def run_lifecycle(handoff, oled, clock, signal_module, cleanup_oled=None, oled_error=None, render_frame=None):
    names = ("Handoff", "Oled", "open_cleanup_oled", "logo_canvas", "render_canvas", "render_housekeeping", "time", "signal")
    saved = {name: getattr(logo, name) for name in names}
    try:
        handoff.clock = clock
        logo.Handoff = types.SimpleNamespace(open=lambda create_lock: handoff)
        logo.Oled = lambda: (_ for _ in ()).throw(oled_error) if oled_error is not None else oled
        logo.open_cleanup_oled = lambda: cleanup_oled if cleanup_oled is not None else (_ for _ in ()).throw(RuntimeError("cleanup factory failed"))
        logo.logo_canvas = lambda kind: kind
        logo.render_canvas = lambda canvas, frame=None: render_frame(frame) if render_frame is not None else b"render"
        logo.render_housekeeping = lambda: render_frame("housekeeping") if render_frame is not None else b"render"
        logo.time = clock
        logo.signal = signal_module
        return logo._run_loop()
    finally:
        for name, value in saved.items():
            setattr(logo, name, value)


def assert_cleanup(events):
    cleanup_events = [event for event in events if event[0] in {"frame", "command", "close"}]
    assert cleanup_events[-3] == ("frame", bytes(32768))
    assert cleanup_events[-2] == ("command", 0xAE)
    assert cleanup_events[-1] == ("close", False)
    assert cleanup_events.count(("close", False)) == 1


stop_at_deadline = 30 * 1_000_000_000
deadline_handoff = LifecycleHandoff(stop_at=stop_at_deadline)
deadline_events = []
deadline_clock = FakeClock()
run_lifecycle(deadline_handoff, LifecycleOled(deadline_events), deadline_clock, FakeSignal())
assert deadline_handoff.release_calls == 1 and deadline_handoff.mark_failed_calls == 0
assert deadline_events[-1] == ("close", False)
assert not any(event[0] == "command" for event in deadline_events)
assert deadline_clock.now >= stop_at_deadline

signal_before_start = FakeSignal()
signal_before_start.trigger_before_start = True
signal_before_start_handoff = LifecycleHandoff()
signal_before_start_events = []
run_lifecycle(signal_before_start_handoff, LifecycleOled(signal_before_start_events), FakeClock(), signal_before_start, LifecycleOled(signal_before_start_events))
assert signal_before_start_handoff.release_calls == 0
assert_cleanup(signal_before_start_events)

start_failure_events = []
start_failure_handoff = LifecycleHandoff(start_error=RuntimeError("start failed"), events=start_failure_events)
start_failure_oled = LifecycleOled(start_failure_events)
try:
    run_lifecycle(start_failure_handoff, start_failure_oled, FakeClock(), FakeSignal(), start_failure_oled)
except RuntimeError as error:
    assert str(error) == "start failed"
else:
    raise AssertionError("handoff.start failure was swallowed")
assert_cleanup(start_failure_events)
assert start_failure_handoff.mark_failed_calls == 1

continuous_handoff = LifecycleHandoff(stop_at=logo.BOOT_SWEEP_DURATION_NS + 1_500_000_000)
continuous_events = []
continuous_clock = FakeClock()
run_lifecycle(
    continuous_handoff,
    LifecycleOled(continuous_events),
    continuous_clock,
    FakeSignal(),
    render_frame=lambda frame: b"clean" if frame is None else b"housekeeping" if frame == "housekeeping" else bytes((frame,)),
)
continuous_frames = [event[1] for event in continuous_events if event[0] == "frame"]
assert logo.BOOT_SWEEP_FRAME_COUNT == 30
assert continuous_frames == [bytes((frame,)) for frame in range(logo.BOOT_SWEEP_FRAME_COUNT)] + [b"clean"]
assert continuous_events.count(("begin-frame-stream",)) == 1
assert continuous_handoff.events.count(("publish-cycle",)) == 0
assert continuous_clock.now == continuous_handoff.stop_at
assert max(continuous_clock.sleeps[16:]) <= logo.BOOT_SWEEP_REST_CHECK_NS / 1_000_000_000
assert continuous_clock.now < logo.BOOT_SWEEP_DURATION_NS + logo.BOOT_SWEEP_REST_NS
assert not any(event[0] == "command" for event in continuous_events)

housekeeping_handoff = LifecycleHandoff(stop_at=31 * 1_000_000_000)
housekeeping_events = []
housekeeping_clock = FakeClock()
run_lifecycle(
    housekeeping_handoff,
    LifecycleOled(housekeeping_events),
    housekeeping_clock,
    FakeSignal(),
    render_frame=lambda frame: b"housekeeping" if frame == "housekeeping" else b"clean" if frame is None else bytes((frame,)),
)
housekeeping_frames = [event[1] for event in housekeeping_events if event[0] == "frame"]
assert housekeeping_handoff.release_calls == 1 and housekeeping_handoff.mark_failed_calls == 0
assert housekeeping_frames.count(b"housekeeping") == 1
assert housekeeping_frames[-1] == b"housekeeping"
assert housekeeping_clock.now == housekeeping_handoff.stop_at

stale_status = {"phase": "animating", "bootId": "01234567-89ab-cdef-0123-456789abcdef"}
stale_events = []
stale_handoff = LifecycleHandoff(start_error=RuntimeError("OLED handoff already exists for this boot"), initial_status=stale_status, events=stale_events)
try:
    run_lifecycle(stale_handoff, LifecycleOled(stale_events), FakeClock(), FakeSignal(), LifecycleOled(stale_events))
except RuntimeError as error:
    assert str(error) == "OLED handoff already exists for this boot"
else:
    raise AssertionError("stale animating handoff was accepted")
assert_cleanup(stale_events)

construction_events = []
construction_handoff = LifecycleHandoff(events=construction_events)
cleanup_oled = LifecycleOled(construction_events)
try:
    run_lifecycle(construction_handoff, None, FakeClock(), FakeSignal(), cleanup_oled, oled_error=RuntimeError("OLED open failed"))
except RuntimeError as error:
    assert str(error) == "OLED open failed"
else:
    raise AssertionError("OLED construction failure was swallowed")
assert_cleanup(construction_events)

factory_failure_events = []
factory_failure_handoff = LifecycleHandoff(events=factory_failure_events)
try:
    run_lifecycle(factory_failure_handoff, None, FakeClock(), FakeSignal(), oled_error=RuntimeError("OLED open failed"))
except RuntimeError as error:
    assert str(error) == "OLED open failed"
else:
    raise AssertionError("cleanup factory failure swallowed the initiating exception")
assert not any(event[0] == "frame" for event in factory_failure_events)
assert factory_failure_handoff.mark_failed_calls == 1

before_deadline_handoff = LifecycleHandoff(stop_at=29 * 1_000_000_000)
before_deadline_events = []
before_deadline_clock = FakeClock()
run_lifecycle(before_deadline_handoff, LifecycleOled(before_deadline_events), before_deadline_clock, FakeSignal())
assert before_deadline_handoff.release_calls == 1 and before_deadline_handoff.mark_failed_calls == 0
assert before_deadline_clock.now < stop_at_deadline

termination_handoff = LifecycleHandoff(stop_at=31 * 1_000_000_000)
termination_events = []
termination_signal = FakeSignal()
termination_clock = FakeClock(termination_signal)
termination_oled = LifecycleOled(termination_events)
termination_stream_frame = termination_oled.stream_frame


def terminate_after_housekeeping(frame):
    return b"housekeeping" if frame == "housekeeping" else b"render"


def terminate_after_housekeeping_status(payload):
    termination_stream_frame(payload)
    if payload == b"housekeeping":
        termination_signal.trigger_on_sleep = True


termination_oled.stream_frame = terminate_after_housekeeping_status


run_lifecycle(
    termination_handoff,
    termination_oled,
    termination_clock,
    termination_signal,
    render_frame=terminate_after_housekeeping,
)
assert termination_handoff.release_calls == 0
assert termination_handoff.mark_failed_calls == 1
assert termination_events.count(("frame", b"housekeeping")) == 1
assert_cleanup(termination_events)

for signum in (signal.SIGTERM, signal.SIGINT):
    signal_module = FakeSignal()
    signal_module.trigger_on_sleep = True
    interrupted_handoff = LifecycleHandoff()
    interrupted_events = []
    interrupted_clock = FakeClock(signal_module, signum)
    run_lifecycle(interrupted_handoff, LifecycleOled(interrupted_events), interrupted_clock, signal_module)
    assert interrupted_handoff.release_calls == 0 and interrupted_handoff.mark_failed_calls == 1
    assert ("command", 0xAE) in interrupted_events and ("close", False) in interrupted_events
    assert interrupted_events.count(("close", False)) == 1
    assert signal_module.handlers[signal_module.SIGTERM] == "default"
    assert signal_module.handlers[signal_module.SIGINT] == "default"

black_failure_handoff = LifecycleHandoff()
black_failure_events = []
try:
    run_lifecycle(
        black_failure_handoff,
        LifecycleOled(black_failure_events, fail_render=True, fail_black=True),
        FakeClock(),
        FakeSignal(),
    )
except RuntimeError:
    pass
else:
    raise AssertionError("black cleanup failure did not preserve timeout")
assert ("command", 0xAE) in black_failure_events and ("close", False) in black_failure_events
assert black_failure_handoff.mark_failed_calls == 1
assert black_failure_events.count(("close", False)) == 1

off_failure_handoff = LifecycleHandoff()
off_failure_events = []
try:
    run_lifecycle(
        off_failure_handoff,
        LifecycleOled(off_failure_events, fail_render=True, fail_off=True),
        FakeClock(),
        FakeSignal(),
    )
except RuntimeError:
    pass
else:
    raise AssertionError("display-off cleanup failure did not preserve timeout")
assert bytes(32768) in [event[1] for event in off_failure_events if event[0] == "frame"]
assert ("command", 0xAE) in off_failure_events and ("close", False) in off_failure_events
assert off_failure_handoff.mark_failed_calls == 1
assert off_failure_events.count(("close", False)) == 1

render_failure_handoff = LifecycleHandoff()
render_failure_events = []
try:
    run_lifecycle(render_failure_handoff, LifecycleOled(render_failure_events, fail_render=True), FakeClock(), FakeSignal())
except RuntimeError as error:
    assert str(error) == "render failed"
else:
    raise AssertionError("render failure was swallowed")
assert render_failure_handoff.mark_failed_calls == 1 and render_failure_events[-1] == ("close", False)
assert render_failure_events.count(("close", False)) == 1

ordering_events = []
ordering_handoff = LifecycleHandoff(stop_at=1, events=ordering_events)
ordering_oled = LifecycleOled(ordering_events)
run_lifecycle(ordering_handoff, ordering_oled, FakeClock(), FakeSignal())
assert [event[0] for event in ordering_events if event[0] in {"release", "close"}] == ["release", "close"]

release_failure_events = []
release_failure_handoff = LifecycleHandoff(stop_at=1, events=release_failure_events, release_error=RuntimeError("release failed"))
release_failure_oled = LifecycleOled(release_failure_events)
try:
    run_lifecycle(release_failure_handoff, release_failure_oled, FakeClock(), FakeSignal())
except RuntimeError as error:
    assert str(error) == "release failed"
else:
    raise AssertionError("release failure was swallowed")
assert_cleanup(release_failure_events)
assert release_failure_handoff.mark_failed_calls == 1

close_failure_events = []
close_failure_handoff = LifecycleHandoff(stop_at=1, events=close_failure_events)
close_failure_oled = LifecycleOled(close_failure_events, fail_close=True)
try:
    run_lifecycle(close_failure_handoff, close_failure_oled, FakeClock(), FakeSignal())
except RuntimeError as error:
    assert str(error) == "close failed"
else:
    raise AssertionError("close-after-release failure was swallowed")
close_failure_sequence = [event[0] for event in close_failure_events if event[0] in {"release", "close", "frame", "command"}]
assert close_failure_sequence[-4:] == ["release", "close", "frame", "command"]
assert close_failure_events.count(("close", False)) == 1
assert close_failure_handoff.mark_failed_calls == 1

terminal_events = []
terminal_handoff = LifecycleHandoff(initial_status={"phase": "native_owned", "bootId": "01234567-89ab-cdef-0123-456789abcdef"}, events=terminal_events)
cleanup_calls = []
run_lifecycle(terminal_handoff, None, FakeClock(), FakeSignal(), cleanup_oled=None)
assert terminal_events == [("handoff-close",)]

print("Orange OLED lifecycle deadline, stop, signal, cleanup, and failure tests passed")
