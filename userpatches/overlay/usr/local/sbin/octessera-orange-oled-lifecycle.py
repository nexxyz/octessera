#!/usr/bin/env python3
import sys


def _sleep_until_frame_or_handoff_deadline(logo, frame_deadline, handoff_deadline):
    logo["_sleep_until"](min(frame_deadline, handoff_deadline))


def _wait_for_rest(logo, handoff, termination_requested, rest_start, handoff_deadline):
    rest_deadline = rest_start + logo["BOOT_SWEEP_REST_NS"]
    while True:
        if handoff.stop_requested():
            return True
        if termination_requested[0]:
            return False
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


def _render_housekeeping_and_wait(logo, oled, handoff, termination_requested, frame):
    if handoff.stop_requested():
        return True
    if termination_requested[0]:
        return False
    oled.stream_frame(frame)
    while True:
        if handoff.stop_requested():
            return True
        if termination_requested[0]:
            return False
        now = logo["time"].monotonic_ns()
        logo["_sleep_until"](now + logo["BOOT_SWEEP_REST_CHECK_NS"])


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
        handoff = logo["Handoff"].open(True)
    except Exception:
        _restore_signal_handlers(logo["signal"], previous_handlers)
        raise
    oled = None
    terminal = False
    released = False
    oled_closed = False
    oled_close_attempted = False
    cleanup_handle_attempted = False
    failed = False

    def close_oled_without_off():
        nonlocal oled_closed, oled_close_attempted
        if oled is not None and not oled_close_attempted:
            oled_close_attempted = True
            oled.close(False)
            oled_closed = True

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

    try:
        status = handoff._read_status()
        if status is not None and status["bootId"] == handoff.boot_id and status["phase"] in logo["NATIVE_HANDOFF_PHASES"]:
            terminal = True
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
        housekeeping_frame = logo["render_housekeeping"]()
        logo["notify_systemd_ready"]()
        oled.begin_frame_stream()
        cycle_start = logo["time"].monotonic_ns()
        frame = 0
        while True:
            _sleep_until_frame_or_handoff_deadline(logo, cycle_start + logo["sweep_deadline_offset_ns"](frame), handoff_deadline)
            if handoff.stop_requested():
                handoff.release()
                close_oled_without_off()
                released = True
                return
            if termination_requested[0]:
                return
            if logo["time"].monotonic_ns() >= handoff_deadline:
                if _render_housekeeping_and_wait(
                    logo,
                    oled,
                    handoff,
                    termination_requested,
                    housekeeping_frame,
                ):
                    handoff.release()
                    close_oled_without_off()
                    released = True
                return
            oled.stream_frame(frames[frame])
            if frame == logo["BOOT_SWEEP_FRAME_COUNT"] - 1:
                if handoff.stop_requested():
                    handoff.release()
                    close_oled_without_off()
                    released = True
                    return
                if termination_requested[0]:
                    return
                cycle_end = cycle_start + logo["BOOT_SWEEP_DURATION_NS"]
                logo["_sleep_until"](min(cycle_end, handoff_deadline))
                if handoff.stop_requested():
                    handoff.release()
                    close_oled_without_off()
                    released = True
                    return
                if termination_requested[0]:
                    return
                if logo["time"].monotonic_ns() >= handoff_deadline:
                    if _render_housekeeping_and_wait(
                        logo,
                        oled,
                        handoff,
                        termination_requested,
                        housekeeping_frame,
                    ):
                        handoff.release()
                        close_oled_without_off()
                        released = True
                    return
                oled.stream_frame(clean_frame)
                if _wait_for_rest(
                    logo,
                    handoff,
                    termination_requested,
                    cycle_end,
                    handoff_deadline,
                ):
                    handoff.release()
                    close_oled_without_off()
                    released = True
                    return
                if logo["time"].monotonic_ns() >= handoff_deadline:
                    if _render_housekeeping_and_wait(
                        logo,
                        oled,
                        handoff,
                        termination_requested,
                        housekeeping_frame,
                    ):
                        handoff.release()
                        close_oled_without_off()
                        released = True
                    return
                next_cycle_start = cycle_start + logo["BOOT_SWEEP_DURATION_NS"] + logo["BOOT_SWEEP_REST_NS"]
                handoff.publish_cycle()
                cycle_start = next_cycle_start
                frame = 0
            else:
                frame += 1
    finally:
        if not terminal and not released:
            fail_handoff_and_oled()
        try:
            handoff.close()
        except Exception as error:
            print(f"Orange OLED handoff close cleanup failed: {error}", file=sys.stderr)
        _restore_signal_handlers(logo["signal"], previous_handlers)
