import signal
import types


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


class NativeRetryActor:
    def __init__(self, outcome="first_menu", fatal_code=None, steps_before_attach=1):
        self.outcome = outcome
        self.fatal_code = fatal_code
        self.steps_before_attach = steps_before_attach
        self.attempts = 0

    def step(self, handoff):
        if self.outcome is None or handoff.native_lock_held or self.steps_before_attach > 0:
            if self.outcome is not None and handoff.observing and not handoff.native_lock_held:
                self.steps_before_attach -= 1
            return
        self.attempts += 1
        handoff.native_lock_held = True
        handoff.events.append(("native-acquire",))
        handoff.fatal_code = self.fatal_code
        request_id = handoff.request_id
        if self.outcome == "first_menu":
            handoff.status = {"phase": "native_owned", "bootId": handoff.boot_id, "requestId": request_id}
            handoff.events.append(("native-owned",))
            handoff.status = {"phase": "first_menu_rendered", "bootId": handoff.boot_id, "requestId": request_id}
            handoff.events.append(("first-menu",))
        elif self.outcome == "failed":
            handoff.status = {"phase": "failed", "bootId": handoff.boot_id, "requestId": request_id}
            handoff.native_lock_held = False
            handoff.events.append(("native-failed",))


class LifecycleHandoff:
    def __init__(self, stop_at=None, start_error=None, initial_status=None, events=None, release_error=None, fatal_code=None, observer_status=None, reacquire_status=None, observer_statuses=None, native_retry=None):
        self.boot_id = "01234567-89ab-cdef-0123-456789abcdef"
        self.stop_at = stop_at
        self.clock = None
        self.status = initial_status
        self.start_error = start_error
        self.events = events if events is not None else []
        self.release_error = release_error
        self.fatal_code = fatal_code
        self.observer_status = observer_status
        self.reacquire_status = reacquire_status
        self.observer_statuses = list(observer_statuses or [])
        self.native_retry = native_retry
        self.observing = False
        self.lock_held = True
        self.native_lock_held = False
        self.request_id = (initial_status or {}).get("requestId")
        self.stop_requested_calls = 0
        self.mark_failed_calls = 0
        self.release_calls = 0
        self.release_existing_calls = 0
        self.reacquire_calls = 0
        self.close_calls = 0

    def _read_status(self):
        if self.observing and self.native_retry is not None:
            self.native_retry.step(self)
        status = self.status
        if self.observing and self.observer_statuses:
            self.status = self.observer_statuses.pop(0)
        return status

    def _read_stop(self):
        if self.request_id is None:
            return None
        return {"bootId": self.boot_id, "requestId": self.request_id}

    def peek_terminal(self):
        status = self.status
        stop = self._read_stop()
        return status is not None and status.get("bootId") == self.boot_id and status.get("phase") == "first_menu_rendered" and stop is not None and status.get("requestId") == stop.get("requestId")

    def startup_fatal_code(self):
        if callable(self.fatal_code):
            return self.fatal_code(self.clock.now)
        return self.fatal_code

    def start(self):
        self.events.append(("start",))
        if self.start_error is not None:
            raise self.start_error
        self.status = {"phase": "animating", "bootId": self.boot_id}

    def stop_requested(self):
        self.stop_requested_calls += 1
        if self.stop_at is None or self.clock.now < self.stop_at:
            return False
        self.request_id = "0123456789abcdef0123456789abcdef"
        self.status = {"phase": "release_requested", "bootId": self.boot_id, "requestId": self.request_id}
        return True

    def publish_cycle(self):
        self.events.append(("publish-cycle",))

    def release(self):
        self.events.append(("release",))
        self.release_calls += 1
        if self.release_error is not None:
            raise self.release_error
        self.observing = True
        self.status = self.observer_status or (self.observer_statuses.pop(0) if self.observer_statuses else {"phase": "first_menu_rendered", "bootId": self.boot_id, "requestId": self.request_id})

    def release_existing(self):
        self.release_existing_calls += 1
        self.events.append(("release-existing",))
        self.status = {"phase": "release_requested", "bootId": self.boot_id, "requestId": self.request_id}
        self.events.append(("release",))
        self.release_calls += 1
        if self.release_error is not None:
            raise self.release_error
        self.observing = True
        self.status = {"phase": "released", "bootId": self.boot_id, "requestId": self.request_id}

    def unlock_preserving(self):
        self.events.append(("unlock",))
        self.lock_held = False

    def reacquire_nonblocking(self, request_id):
        self.reacquire_calls += 1
        self.events.append(("reacquire",))
        if self.native_lock_held:
            return None
        reacquire_status = self.reacquire_status
        if isinstance(reacquire_status, list):
            reacquire_status = reacquire_status.pop(0) if reacquire_status else None
        if reacquire_status is None:
            return None
        self.lock_held = True
        self.status = reacquire_status
        return self.status

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
        if payload == bytes(32768) and self.fail_black:
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


def run_lifecycle(logo, handoff, oled, clock, signal_module, cleanup_oled=None, oled_error=None, render_frame=None, oled_factory=None):
    names = ("Handoff", "Oled", "open_cleanup_oled", "logo_canvas", "render_canvas", "render_startup_delayed", "render_startup_fatal", "notify_systemd_ready", "time", "signal")
    saved = {name: getattr(logo, name) for name in names}
    try:
        handoff.clock = clock
        logo.Handoff = types.SimpleNamespace(open=lambda create_lock: handoff, peek_terminal=handoff.peek_terminal)
        logo.Oled = lambda: (_ for _ in ()).throw(oled_error) if oled_error is not None else oled_factory() if oled_factory is not None else oled
        logo.open_cleanup_oled = lambda: cleanup_oled if cleanup_oled is not None else (_ for _ in ()).throw(RuntimeError("cleanup factory failed"))
        logo.logo_canvas = lambda kind: kind
        logo.render_canvas = lambda canvas, frame=None: render_frame(frame) if render_frame is not None else b"render"
        logo.render_startup_delayed = lambda: render_frame("delayed") if render_frame is not None else b"render"
        logo.render_startup_fatal = lambda code, dimmed=False: render_frame(("fatal", code, dimmed)) if render_frame is not None else b"render"
        logo.notify_systemd_ready = lambda: handoff.events.append(("ready",))
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
