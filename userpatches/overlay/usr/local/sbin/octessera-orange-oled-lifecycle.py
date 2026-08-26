#!/usr/bin/env python3
import sys


FATAL_DIM_DELAY_NS = 60 * 1_000_000_000


def _sleep_until_frame_or_handoff_deadline(logo, frame_deadline, handoff_deadline):
    logo["_sleep_until"](min(frame_deadline, handoff_deadline))


def _stream_frame(logo, oled, frame, readiness):
    oled.stream_frame(frame)
    if not readiness[0]:
        logo["notify_systemd_ready"]()
        readiness[0] = True


def _render_fatal_if_changed(logo, oled, fatal_code, displayed, fatal_state, now, readiness):
    if fatal_state[0] != fatal_code:
        _stream_frame(logo, oled, logo["render_startup_fatal"](fatal_code, dimmed=False), readiness)
        fatal_state[:] = [fatal_code, now, False]
        displayed[0] = ("fatal", fatal_code, False)
    elif not fatal_state[2] and now - fatal_state[1] >= FATAL_DIM_DELAY_NS:
        _stream_frame(logo, oled, logo["render_startup_fatal"](fatal_code, dimmed=True), readiness)
        fatal_state[2] = True
        displayed[0] = ("fatal", fatal_code, True)


def _render_frame_if_changed(logo, oled, frame, marker, displayed, readiness):
    if displayed[0] != marker:
        _stream_frame(logo, oled, frame, readiness)
        displayed[0] = marker


def _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness):
    if handoff.stop_requested():
        return "stop"
    fatal_code = handoff.startup_fatal_code()
    if fatal_code is not None:
        now = logo["time"].monotonic_ns()
        _render_fatal_if_changed(logo, oled, fatal_code, displayed, fatal_state, now, readiness)
        return "fatal"
    fatal_state[:] = [None, None, False]
    return "continue"


def _wait_for_fatal(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness):
    while True:
        state = _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness)
        if state == "stop":
            return "stop"
        if termination_requested[0]:
            return "terminate"
        if state == "continue":
            return "clear"
        now = logo["time"].monotonic_ns()
        logo["_sleep_until"](now + logo["BOOT_SWEEP_REST_CHECK_NS"])


def _wait_for_animation_tail(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness, cycle_end, handoff_deadline):
    while True:
        state = _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness)
        if state == "stop":
            return "stop"
        if termination_requested[0]:
            return "terminate"
        if state == "fatal":
            outcome = _wait_for_fatal(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness)
            if outcome != "clear":
                return outcome
        now = logo["time"].monotonic_ns()
        if now >= handoff_deadline:
            return "deadline"
        if now >= cycle_end:
            return "cycle_end"
        logo["_sleep_until"](
            min(
                cycle_end,
                handoff_deadline,
                now + logo["BOOT_SWEEP_REST_CHECK_NS"],
            )
        )


def _wait_for_rest(logo, oled, handoff, termination_requested, rest_start, handoff_deadline, displayed, fatal_state, readiness, clean_frame):
    rest_deadline = rest_start + logo["BOOT_SWEEP_REST_NS"]
    while True:
        state = _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness)
        if state == "stop":
            return True
        if termination_requested[0]:
            return False
        if state == "fatal":
            outcome = _wait_for_fatal(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness)
            if outcome == "stop":
                return True
            if outcome == "terminate":
                return False
            _render_frame_if_changed(logo, oled, clean_frame, ("clean",), displayed, readiness)
            continue
        now = logo["time"].monotonic_ns()
        if now >= handoff_deadline:
            return False
        if now >= rest_deadline:
            return False
        next_deadline = min(
            rest_deadline,
            handoff_deadline,
            now + logo["BOOT_SWEEP_REST_CHECK_NS"],
        )
        logo["_sleep_until"](next_deadline)


def _render_startup_delayed_and_wait(logo, oled, handoff, termination_requested, frame, displayed, fatal_state, readiness):
    while True:
        state = _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness)
        if state == "stop":
            return True
        if termination_requested[0]:
            return False
        if state == "fatal":
            outcome = _wait_for_fatal(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness)
            if outcome == "stop":
                return True
            if outcome == "terminate":
                return False
            continue
        _render_frame_if_changed(logo, oled, frame, ("delayed",), displayed, readiness)
        now = logo["time"].monotonic_ns()
        logo["_sleep_until"](now + logo["BOOT_SWEEP_REST_CHECK_NS"])


def _reclaim_fatal_code(code):
    return code if code is not None else "startup_failed"


def _status_is_first_menu(status, handoff, request_id):
    if status is None or status.get("bootId") != handoff.boot_id or status.get("phase") != "first_menu_rendered" or status.get("requestId") != request_id:
        return False
    stop = handoff._read_stop()
    return stop is not None and stop.get("bootId") == handoff.boot_id and stop.get("requestId") == request_id


def _observer_state(logo, handoff, request_id, fatal_state, force_probe):
    status = handoff._read_status()
    if _status_is_first_menu(status, handoff, request_id):
        return "terminal"
    if status is None or status.get("bootId") != handoff.boot_id:
        return "continue"
    phase = status.get("phase")
    if phase in {"native_owned", "failed"}:
        return "reclaim"
    if phase != "released":
        return "continue"
    fatal_code = handoff.startup_fatal_code()
    displayed_code = _reclaim_fatal_code(fatal_code)
    if force_probe or (fatal_code is not None and fatal_state[0] != displayed_code):
        return "reclaim"
    if fatal_state[0] == displayed_code and fatal_state[1] is not None and not fatal_state[2]:
        now = logo["time"].monotonic_ns()
        if now - fatal_state[1] >= FATAL_DIM_DELAY_NS:
            return "reclaim-dim"
    return "continue"


def _wait_for_observer(logo, handoff, termination_requested, request_id, fatal_state, force_probe):
    while True:
        if termination_requested[0]:
            return "terminate"
        state = _observer_state(logo, handoff, request_id, fatal_state, force_probe)
        if state in {"terminal", "reclaim", "reclaim-dim"}:
            return state
        now = logo["time"].monotonic_ns()
        logo["_sleep_until"](now + logo["BOOT_SWEEP_REST_CHECK_NS"])


def _reclaim_fatal(logo, oled, handoff, displayed, fatal_state, readiness, dimmed):
    fatal_code = _reclaim_fatal_code(handoff.startup_fatal_code())
    now = logo["time"].monotonic_ns()
    if dimmed and fatal_state[0] == fatal_code and fatal_state[1] is not None and not fatal_state[2]:
        _stream_frame(logo, oled, logo["render_startup_fatal"](fatal_code, dimmed=True), readiness)
        fatal_state[2] = True
        displayed[0] = ("fatal", fatal_code, True)
    else:
        _render_fatal_if_changed(logo, oled, fatal_code, displayed, fatal_state, now, readiness)


def _install_signal_handlers(termination_requested):
    signal_module = termination_requested[1]
    previous_handlers = {}

    def request_termination(_signum, _frame):
        termination_requested[0] = True

    try:
        for signum in (signal_module.SIGTERM, signal_module.SIGINT):
            previous_handlers[signum] = signal_module.getsignal(signum)
            signal_module.signal(signum, request_termination)
    except Exception:
        for signum, handler in previous_handlers.items():
            signal_module.signal(signum, handler)
        raise
    return previous_handlers


def _restore_signal_handlers(signal_module, previous_handlers):
    for signum, handler in previous_handlers.items():
        try:
            signal_module.signal(signum, handler)
        except Exception as error:
            print(f"Orange OLED signal handler restore failed: {error}", file=sys.stderr)


def run_loop(logo):
    termination_requested = [False, logo["signal"]]
    previous_handlers = _install_signal_handlers(termination_requested)
    try:
        if logo["Handoff"].peek_terminal():
            logo["notify_systemd_ready"]()
            _restore_signal_handlers(logo["signal"], previous_handlers)
            return
        handoff = logo["Handoff"].open(True)
    except Exception:
        _restore_signal_handlers(logo["signal"], previous_handlers)
        raise
    oled = None
    terminal = False
    released = False
    observer = False
    oled_closed = False
    oled_close_attempted = False
    cleanup_handle_attempted = False
    failed = False
    readiness = [False]

    def close_oled_without_off():
        nonlocal oled_closed, oled_close_attempted
        if oled is not None and not oled_close_attempted:
            oled_close_attempted = True
            oled.close(False)
            oled_closed = True

    def release_handoff():
        nonlocal released, observer
        handoff.release()
        close_oled_without_off()
        handoff.unlock_preserving()
        released = True
        observer = True

    def retry_release():
        nonlocal released, observer
        handoff.release_existing()
        close_oled_without_off()
        handoff.unlock_preserving()
        released = True
        observer = True

    def fail_handoff_and_oled():
        nonlocal failed, oled, oled_closed, oled_close_attempted, cleanup_handle_attempted
        if oled is None and not cleanup_handle_attempted:
            cleanup_handle_attempted = True
            try:
                oled = logo["open_cleanup_oled"]()
            except Exception as error:
                print(f"Orange OLED cleanup handle open failed: {error}", file=sys.stderr)
        if oled is not None and not oled_closed:
            operations = (
                (lambda: getattr(oled, "frame")(bytes(logo["WIDTH"] * logo["HEIGHT"] * 2)), "black"),
                (lambda: getattr(oled, "command")(0xAE), "display-off"),
            )
            for operation, name in operations:
                try:
                    operation()
                except Exception as error:
                    print(f"Orange OLED {name} cleanup failed: {error}", file=sys.stderr)
            if not oled_close_attempted:
                oled_close_attempted = True
                try:
                    oled.close(False)
                except Exception as error:
                    print(f"Orange OLED close cleanup failed: {error}", file=sys.stderr)
            oled_closed = True
        if not failed:
            failed = True
            try:
                handoff.mark_failed()
            except Exception as error:
                print(f"Orange OLED handoff failure cleanup failed: {error}", file=sys.stderr)

    def observe_after_release(force_probe=False, initial_fatal_state=None):
        nonlocal oled, released, observer, oled_closed, oled_close_attempted, cleanup_handle_attempted
        request_id = handoff.request_id
        if request_id is None:
            raise RuntimeError("OLED observer has no release request")
        reclaimed_state = list(initial_fatal_state or [None, None, False])
        while True:
            outcome = _wait_for_observer(logo, handoff, termination_requested, request_id, reclaimed_state, force_probe)
            if outcome == "terminal":
                return "terminal"
            if outcome == "terminate":
                return "terminate"
            if termination_requested[0]:
                return "terminate"
            status = handoff.reacquire_nonblocking(request_id)
            if status is None:
                continue
            if _status_is_first_menu(status, handoff, request_id):
                handoff.unlock_preserving()
                return "terminal"
            released = False
            observer = False
            fatal_code = _reclaim_fatal_code(handoff.startup_fatal_code())
            native_reclaim = status.get("phase") in {"native_owned", "failed"}
            if native_reclaim:
                reclaimed_state[:] = [None, None, False]
            should_dim = outcome == "reclaim-dim" and reclaimed_state[0] == fatal_code
            should_render = reclaimed_state[0] != fatal_code or should_dim
            if should_render:
                oled = logo["Oled"]()
                oled_closed = False
                oled_close_attempted = False
                cleanup_handle_attempted = False
                oled.begin_frame_stream()
                reclaimed_displayed = [None]
                _reclaim_fatal(logo, oled, handoff, reclaimed_displayed, reclaimed_state, readiness, should_dim)
            retry_release()
            force_probe = False

    def notify_ready():
        if not readiness[0]:
            logo["notify_systemd_ready"]()
            readiness[0] = True

    def finish_release():
        nonlocal terminal
        release_handoff()
        outcome = observe_after_release(initial_fatal_state=fatal_state)
        if outcome == "terminal":
            terminal = True
            notify_ready()

    try:
        status = handoff._read_status()
        if status is not None and status["bootId"] == handoff.boot_id and status["phase"] != "animating":
            request_id = status.get("requestId")
            if request_id is None:
                raise RuntimeError("OLED existing handoff request is missing")
            if _status_is_first_menu(status, handoff, request_id):
                terminal = True
                notify_ready()
                return
            handoff.request_id = request_id
            handoff.release_existing()
            handoff.unlock_preserving()
            released = True
            observer = True
            outcome = observe_after_release(True)
            if outcome == "terminal":
                terminal = True
                notify_ready()
            return
        if termination_requested[0]:
            fail_handoff_and_oled()
            return
        handoff.start()
        handoff_deadline = logo["time"].monotonic_ns() + logo["NATIVE_HANDOFF_TIMEOUT_SECONDS"] * 1_000_000_000
        oled = logo["Oled"]()
        oled.initialize()
        canvas = logo["logo_canvas"]("boot")
        frames = [
            logo["render_canvas"](canvas, frame)
            for frame in range(logo["BOOT_SWEEP_FRAME_COUNT"])
        ]
        clean_frame = logo["render_canvas"](canvas)
        delayed_start_frame = logo["render_startup_delayed"]()
        oled.begin_frame_stream()
        cycle_start = logo["time"].monotonic_ns()
        frame = 0
        displayed = [None]
        fatal_state = [None, None, False]
        while True:
            _sleep_until_frame_or_handoff_deadline(logo, cycle_start + logo["sweep_deadline_offset_ns"](frame), handoff_deadline)
            state = _poll_startup_state(logo, oled, handoff, displayed, fatal_state, readiness)
            if state == "stop":
                finish_release()
                return
            if termination_requested[0]:
                return
            if state == "fatal":
                outcome = _wait_for_fatal(logo, oled, handoff, termination_requested, displayed, fatal_state, readiness)
                if outcome == "stop":
                    finish_release()
                    return
                if outcome == "terminate":
                    return
            if logo["time"].monotonic_ns() >= handoff_deadline:
                if _render_startup_delayed_and_wait(
                    logo,
                    oled,
                    handoff,
                    termination_requested,
                    delayed_start_frame,
                    displayed,
                    fatal_state,
                    readiness,
                ):
                    finish_release()
                return
            _stream_frame(logo, oled, frames[frame], readiness)
            displayed[0] = ("sweep", frame)
            if frame == logo["BOOT_SWEEP_FRAME_COUNT"] - 1:
                tail = _wait_for_animation_tail(
                    logo,
                    oled,
                    handoff,
                    termination_requested,
                    displayed,
                    fatal_state,
                    readiness,
                    cycle_start + logo["BOOT_SWEEP_DURATION_NS"],
                    handoff_deadline,
                )
                if tail == "stop":
                    finish_release()
                    return
                if tail == "terminate":
                    return
                if tail == "deadline":
                    if _render_startup_delayed_and_wait(
                        logo,
                        oled,
                        handoff,
                        termination_requested,
                        delayed_start_frame,
                        displayed,
                        fatal_state,
                        readiness,
                    ):
                        finish_release()
                    return
                _render_frame_if_changed(logo, oled, clean_frame, ("clean",), displayed, readiness)
                if _wait_for_rest(
                    logo,
                    oled,
                    handoff,
                    termination_requested,
                    cycle_start + logo["BOOT_SWEEP_DURATION_NS"],
                    handoff_deadline,
                    displayed,
                    fatal_state,
                    readiness,
                    clean_frame,
                ):
                    finish_release()
                    return
                if logo["time"].monotonic_ns() >= handoff_deadline:
                    if _render_startup_delayed_and_wait(
                        logo,
                        oled,
                        handoff,
                        termination_requested,
                        delayed_start_frame,
                        displayed,
                        fatal_state,
                        readiness,
                    ):
                        finish_release()
                    return
                next_cycle_start = cycle_start + logo["BOOT_SWEEP_DURATION_NS"] + logo["BOOT_SWEEP_REST_NS"]
                handoff.publish_cycle()
                cycle_start = next_cycle_start
                frame = 0
            else:
                frame += 1
    finally:
        if not terminal and not released and not observer:
            fail_handoff_and_oled()
        try:
            handoff.close()
        except Exception as error:
            print(f"Orange OLED handoff close cleanup failed: {error}", file=sys.stderr)
        _restore_signal_handlers(logo["signal"], previous_handlers)
