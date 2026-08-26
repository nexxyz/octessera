#!/usr/bin/env python3
import importlib.util
import signal
import sys
import types
from importlib.machinery import SourceFileLoader
from pathlib import Path
from orange_oled_lifecycle_test_support import FakeClock, FakeSignal, LifecycleHandoff, LifecycleOled, NativeRetryActor, assert_cleanup as assert_cleanup_events, run_lifecycle as run_lifecycle_with_logo


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
assert logo._lifecycle_module.FATAL_DIM_DELAY_NS == 60 * 1_000_000_000
assert [logo.sweep_deadline_offset_ns(frame) for frame in range(30)] == [frame * 40_000_000 for frame in range(30)]


def run_lifecycle(*args, **kwargs):
    return run_lifecycle_with_logo(logo, *args, **kwargs)


def assert_cleanup(events):
    assert_cleanup_events(events)


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

continuous_events = []
continuous_handoff = LifecycleHandoff(stop_at=logo.BOOT_SWEEP_DURATION_NS + 1_500_000_000, events=continuous_events)
continuous_clock = FakeClock()
run_lifecycle(
    continuous_handoff,
    LifecycleOled(continuous_events),
    continuous_clock,
    FakeSignal(),
    render_frame=lambda frame: b"clean" if frame is None else b"delayed" if frame == "delayed" else bytes((frame,)),
)
continuous_frames = [event[1] for event in continuous_events if event[0] == "frame"]
assert logo.BOOT_SWEEP_FRAME_COUNT == 30
assert continuous_frames == [bytes((frame,)) for frame in range(logo.BOOT_SWEEP_FRAME_COUNT)] + [b"clean"]
assert continuous_events.count(("begin-frame-stream",)) == 1
first_frame_index = next(index for index, event in enumerate(continuous_events) if event[0] == "frame")
ready_index = next(index for index, event in enumerate(continuous_events) if event[0] == "ready")
assert continuous_events.index(("start",)) < first_frame_index < ready_index
assert continuous_events.count(("ready",)) == 1
assert continuous_handoff.events.count(("publish-cycle",)) == 0
assert continuous_clock.now == continuous_handoff.stop_at
assert max(continuous_clock.sleeps[16:]) <= logo.BOOT_SWEEP_REST_CHECK_NS / 1_000_000_000
assert continuous_clock.now < logo.BOOT_SWEEP_DURATION_NS + logo.BOOT_SWEEP_REST_NS
assert not any(event[0] == "command" for event in continuous_events)


def fatal_render(frame):
    if isinstance(frame, tuple) and frame[0] == "fatal":
        return f"dimmed:{frame[1]}".encode() if frame[2] else f"fatal:{frame[1]}".encode()
    return b"delayed" if frame == "delayed" else b"clean" if frame is None else bytes((frame,))


def run_fatal_schedule(fatal_code, stop_at):
    handoff = LifecycleHandoff(stop_at=stop_at, fatal_code=fatal_code)
    events = []
    clock = FakeClock()
    run_lifecycle(handoff, LifecycleOled(events), clock, FakeSignal(), render_frame=fatal_render)
    frames = [event[1] for event in events if event[0] == "frame"]
    return handoff, frames, clock, events


priority_events = []
priority_handoff = LifecycleHandoff(stop_at=0, events=priority_events, fatal_code="trellis_unavailable")
run_lifecycle(priority_handoff, LifecycleOled(priority_events), FakeClock(), FakeSignal(), render_frame=fatal_render)
assert priority_handoff.release_calls == 1 and not any(event[0] == "frame" and event[1].startswith(b"fatal:") for event in priority_events)


immediate_fatal_handoff, immediate_fatal_frames, immediate_fatal_clock, immediate_fatal_events = run_fatal_schedule(
    lambda now: "trellis_unavailable" if 1_000_000_000 <= now < 1_500_000_000 else None,
    2_000_000_000,
)
assert b"fatal:trellis_unavailable" in immediate_fatal_frames
assert immediate_fatal_handoff.release_calls == 1
assert immediate_fatal_clock.now < 30 * 1_000_000_000
assert not any(event[0] == "command" for event in immediate_fatal_events)

before_dim_handoff, before_dim_frames, before_dim_clock, _ = run_fatal_schedule(
    lambda now: "audio_unavailable" if now < 59_999_000_000 else None,
    60_000_000_000,
)
assert before_dim_frames.count(b"dimmed:audio_unavailable") == 0 and before_dim_handoff.release_calls == 1 and before_dim_clock.now == 60_000_000_000

same_code_handoff, same_code_frames, same_code_clock, same_code_events = run_fatal_schedule(
    lambda now: "audio_unavailable" if now < 120_050_000_000 else None,
    120_100_000_000,
)
assert same_code_frames.count(b"fatal:audio_unavailable") == 1 and same_code_frames.count(b"dimmed:audio_unavailable") == 1
assert same_code_handoff.release_calls == 1 and same_code_clock.now == 120_100_000_000
assert not any(event[0] == "command" for event in same_code_events)

replacement_handoff, replacement_frames, _, _ = run_fatal_schedule(
    lambda now: "trellis_unavailable" if now < 30_000_000_000 else "audio_unavailable" if now < 90_050_000_000 else None,
    90_100_000_000,
)
assert replacement_frames.count(b"fatal:trellis_unavailable") == 1
assert replacement_frames.count(b"fatal:audio_unavailable") == 1
assert replacement_frames.count(b"dimmed:trellis_unavailable") == 0
assert replacement_frames.count(b"dimmed:audio_unavailable") == 1
assert replacement_handoff.release_calls == 1

malformed_fatal_handoff, malformed_fatal_frames, _, _ = run_fatal_schedule(
    lambda now: "startup_failed" if now < 1_000_000_000 else None,
    2_000_000_000,
)
assert malformed_fatal_frames.count(b"fatal:startup_failed") == 1
assert malformed_fatal_handoff.release_calls == 1

clear_recovery_handoff, clear_recovery_frames, _, _ = run_fatal_schedule(
    lambda now: "neokey_unavailable" if now < 10_000_000_000 or 20_000_000_000 <= now < 20_050_000_000 else None,
    21_000_000_000,
)
assert clear_recovery_frames.count(b"fatal:neokey_unavailable") == 2
assert clear_recovery_frames.count(b"dimmed:neokey_unavailable") == 0
assert clear_recovery_handoff.release_calls == 1


request_id = "0123456789abcdef0123456789abcdef"
wrong_terminal = {"phase": "first_menu_rendered", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": "fedcba9876543210fedcba9876543210"}
matching_terminal = {"phase": "first_menu_rendered", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id}
exact_events = []
exact_handoff = LifecycleHandoff(stop_at=1, events=exact_events, observer_statuses=[wrong_terminal, matching_terminal])
exact_oled = LifecycleOled(exact_events)
run_lifecycle(exact_handoff, exact_oled, FakeClock(), FakeSignal())
assert exact_handoff.reacquire_calls == 0
assert exact_events.count(("unlock",)) == 1 and [event[0] for event in exact_events if event[0] in {"release", "close", "unlock"}] == ["release", "close", "unlock"]
assert not any(event[0] == "command" for event in exact_events)


busy_events = []
busy_status = {"phase": "native_owned", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id, "pid": 4242}
busy_handoff = LifecycleHandoff(stop_at=1, events=busy_events, observer_status=busy_status, reacquire_status=[None, matching_terminal])
busy_oled = LifecycleOled(busy_events)
run_lifecycle(busy_handoff, busy_oled, FakeClock(), FakeSignal())
busy_unlock_index = busy_events.index(("unlock",))
assert busy_handoff.reacquire_calls == 2 and not any(event[0] in {"initialize", "begin-frame-stream", "frame", "command"} for event in busy_events[busy_unlock_index + 1:])


retry_events = []
retry_handoff = LifecycleHandoff(
    stop_at=1_000_000_000,
    events=retry_events,
    observer_status={"phase": "failed", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id},
    reacquire_status={"phase": "failed", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id},
)
retry_oled_events = []
retry_initial_oled = LifecycleOled(retry_events)
retry_reclaimed_oled = LifecycleOled(retry_oled_events)
retry_clock = FakeClock()
retry_signal = FakeSignal()
retry_handoff.fatal_code = "oled_unavailable"
retry_handoff.native_retry = NativeRetryActor(outcome="first_menu", fatal_code=None)
run_lifecycle(
    retry_handoff,
    retry_initial_oled,
    retry_clock,
    retry_signal,
    render_frame=fatal_render,
    oled_factory=lambda: retry_reclaimed_oled,
)
assert retry_handoff.release_existing_calls == 1 and retry_handoff.request_id == request_id and retry_handoff.status == matching_terminal
assert b"fatal:oled_unavailable" in [event[1] for event in retry_oled_events if event[0] == "frame"]
assert not any(event[0] == "command" for event in retry_oled_events) and bytes(32768) not in [event[1] for event in retry_oled_events if event[0] == "frame"] and retry_events.count(("unlock",)) == 2
assert retry_events.index(("release-existing",)) < retry_events.index(("unlock",), retry_events.index(("release-existing",))) < retry_events.index(("native-acquire",))


death_events = []
death_handoff = LifecycleHandoff(
    stop_at=1_000_000_000,
    events=death_events,
    observer_status=busy_status,
    reacquire_status=busy_status,
)
death_signal = FakeSignal()
death_clock = FakeClock(death_signal)
death_reclaimed_oled = LifecycleOled(death_events)
death_stream = death_reclaimed_oled.stream_frame


def stop_after_generic_fatal(payload):
    death_stream(payload)
    if payload == b"fatal:startup_failed":
        death_signal.trigger_on_sleep = True


death_reclaimed_oled.stream_frame = stop_after_generic_fatal
run_lifecycle(
    death_handoff,
    LifecycleOled(death_events),
    death_clock,
    death_signal,
    render_frame=fatal_render,
    oled_factory=lambda: death_reclaimed_oled,
)
assert b"fatal:startup_failed" in [event[1] for event in death_events if event[0] == "frame"]
assert death_handoff.mark_failed_calls == 0 and not any(event[0] == "command" for event in death_events)


dim_events = []
dim_handoff = LifecycleHandoff(
    stop_at=1_000_000_000,
    events=dim_events,
    observer_status={"phase": "failed", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id},
    reacquire_status=[{"phase": "failed", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id}, {"phase": "released", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id}],
)
dim_signal = FakeSignal()
dim_clock = FakeClock(dim_signal)
dim_reclaimed_oled = LifecycleOled(dim_events)
dim_stream = dim_reclaimed_oled.stream_frame


def stop_after_dim(payload):
    dim_stream(payload)
    if payload == b"dimmed:startup_failed":
        dim_signal.trigger_on_sleep = True


dim_reclaimed_oled.stream_frame = stop_after_dim
run_lifecycle(
    dim_handoff,
    LifecycleOled(dim_events),
    dim_clock,
    dim_signal,
    render_frame=fatal_render,
    oled_factory=lambda: dim_reclaimed_oled,
)
dim_frames = [event[1] for event in dim_events if event[0] == "frame"]
assert dim_frames.count(b"fatal:startup_failed") == 1 and dim_frames.count(b"dimmed:startup_failed") == 1 and dim_handoff.mark_failed_calls == 0


for signum in (signal.SIGTERM, signal.SIGINT):
    observer_signal = FakeSignal()
    observer_signal.trigger_on_sleep = True
    observer_events = []
    observer_handoff = LifecycleHandoff(
        stop_at=1,
        events=observer_events,
        observer_status={"phase": "native_owned", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id},
    )
    observer_clock = FakeClock(observer_signal, signum)
    run_lifecycle(observer_handoff, LifecycleOled(observer_events), observer_clock, observer_signal)
    assert observer_handoff.mark_failed_calls == 0 and not any(event[0] == "command" for event in observer_events) and observer_events.count(("close", False)) == 1


startup_delay_handoff = LifecycleHandoff(stop_at=31 * 1_000_000_000)
startup_delay_events = []
startup_delay_clock = FakeClock()
run_lifecycle(
    startup_delay_handoff,
    LifecycleOled(startup_delay_events),
    startup_delay_clock,
    FakeSignal(),
    render_frame=lambda frame: b"delayed" if frame == "delayed" else b"clean" if frame is None else bytes((frame,)),
)
startup_delay_frames = [event[1] for event in startup_delay_events if event[0] == "frame"]
assert startup_delay_handoff.release_calls == 1 and startup_delay_handoff.mark_failed_calls == 0
assert startup_delay_frames.count(b"delayed") == 1
assert startup_delay_frames[-1] == b"delayed"
assert startup_delay_handoff.stop_requested_calls > 1
assert not any(event[0] == "command" for event in startup_delay_events)
assert startup_delay_events.count(("close", False)) == 1
assert startup_delay_clock.now == startup_delay_handoff.stop_at

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


def terminate_after_startup_delayed(frame):
    return b"delayed" if frame == "delayed" else b"render"


def terminate_after_startup_delayed_status(payload):
    termination_stream_frame(payload)
    if payload == b"delayed":
        termination_signal.trigger_on_sleep = True


termination_oled.stream_frame = terminate_after_startup_delayed_status


run_lifecycle(
    termination_handoff,
    termination_oled,
    termination_clock,
    termination_signal,
    render_frame=terminate_after_startup_delayed,
)
assert termination_handoff.release_calls == 0
assert termination_handoff.mark_failed_calls == 1
assert termination_events.count(("frame", b"delayed")) == 1
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
terminal_handoff = LifecycleHandoff(initial_status={"phase": "first_menu_rendered", "bootId": "01234567-89ab-cdef-0123-456789abcdef", "requestId": request_id}, events=terminal_events)
run_lifecycle(terminal_handoff, None, FakeClock(), FakeSignal(), cleanup_oled=None)
assert terminal_events == [("ready",)] and terminal_handoff.close_calls == 0 and terminal_handoff.status["phase"] == "first_menu_rendered"

print("Orange OLED lifecycle deadline, stop, signal, cleanup, and failure tests passed")
