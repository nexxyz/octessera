#!/usr/bin/env python3


def active_command(status, operation):
    descriptor = status.lock()
    try:
        record = status.read_active()
        if not status.active_valid(record):
            if record is not None:
                status.remove_active(record)
            return 1
        return operation(record)
    finally:
        status.unlock(descriptor)


def fail_pending(status):
    descriptor = status.lock()
    try:
        record = status.read_active()
        if record is None or record["servicePid"] != 0 or status.inspect_unit_state() in {"active", "activating"}:
            return 0
        status.finish_failed(record, "operation_failed")
        return 0
    finally:
        status.unlock(descriptor)


def main(status, argv):
    if len(argv) < 2:
        return 2
    command = argv[1]
    if command == "start-or-attach" and len(argv) == 5:
        return status.start_or_attach(argv[2], argv[3] == "1", argv[4])
    if command == "start-failed" and len(argv) == 3:
        return status.start_failed(argv[2])
    if command == "fail-pending" and len(argv) == 2:
        return fail_pending(status)
    if command == "ensure-firstboot" and len(argv) == 2:
        return status.ensure_firstboot()
    if command == "active" and len(argv) == 2:
        return active_command(status, lambda record: print(record["attemptId"]) or 0)
    if command == "active-info" and len(argv) == 2:
        return active_command(status, lambda record: print("1" if record["reentry"] else "0") or 0)
    if command == "remaining" and len(argv) == 2:
        return active_command(status, lambda record: print(f"{max(0.0, record['deadlineMonotonic'] - status.time.monotonic()):.3f}") or 0)
    if command == "bind-pid" and len(argv) == 3:
        ticks = status.process_ticks(int(argv[2]))
        if ticks is None:
            return 1
        def bind(record):
            record["servicePid"] = int(argv[2])
            record["serviceStartTicks"] = ticks
            status.write_active(record)
            return 0
        return active_command(status, bind)
    if command in {"update", "terminal"} and len(argv) in {4, 5}:
        phase, error_code = argv[2], argv[3]
        portal_suffix = argv[4] if len(argv) == 5 else None
        if command == "update":
            return status.update_status(command, phase, error_code, portal_suffix)
        return status.terminal_status(phase, error_code, portal_suffix)
    if command == "stop" and len(argv) == 2:
        return status.stop()
    return 2
