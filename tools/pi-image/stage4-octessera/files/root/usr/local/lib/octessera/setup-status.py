#!/usr/bin/env python3
try:
    import fcntl
except ImportError:
    fcntl = None
try:
    import grp
except ImportError:
    grp = None
import importlib.util
import json
import os
import re
import secrets
import stat
import subprocess
import sys
import time


CONTROL_DIR = "/run/octessera-setup-control"
PUBLIC_DIR = "/run/octessera-setup-status"
RECEIPT_DIR = os.path.join(PUBLIC_DIR, "receipts")
LOCK_PATH = os.path.join(CONTROL_DIR, "status.lock")
ACTIVE_PATH = os.path.join(CONTROL_DIR, "active.json")
SEQUENCE_PATH = os.path.join(CONTROL_DIR, "sequence")
MARKER_PATH = "/var/lib/octessera/setup-complete"
BOOT_ID_PATH = "/proc/sys/kernel/random/boot_id"
SETUP_UNIT = "octessera-setup.service"
PUBLIC_GROUP = "pi"
SCHEMA = 1
ATTEMPT_RE = re.compile(r"^[0-9a-f]{32}$")
TOKEN_RE = re.compile(r"^[0-9a-f]{32}$")
BOOT_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
ACTIVE_FIELDS = {"schema", "bootId", "attemptId", "requestToken", "sequence", "reentry", "priorSetupComplete", "startedMonotonic", "deadlineMonotonic", "servicePid", "serviceStartTicks", "claimPath"}
PHASES = {"starting", "portal_ready", "finalizing", "succeeded", "failed", "timed_out", "unsupported"}
DISPOSITIONS = {"accepted", "already_running"}
FAILURE_CODES = {"operation_failed", "unavailable", "invalid_payload"}


def group_id():
    if grp is None:
        raise RuntimeError("group lookup is required")
    return grp.getgrnam(PUBLIC_GROUP).gr_gid


def ensure_dir(path, uid, gid, mode):
    if os.path.lexists(path):
        metadata = os.lstat(path)
        if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != uid or metadata.st_gid != gid or stat.S_IMODE(metadata.st_mode) != mode:
            raise RuntimeError("unsafe setup directory")
        return
    os.mkdir(path, mode)
    os.chown(path, uid, gid)
    os.chmod(path, mode)


def ensure_control():
    ensure_dir(CONTROL_DIR, 0, 0, 0o700)


def ensure_public():
    gid = group_id()
    ensure_dir(PUBLIC_DIR, 0, gid, 0o750)
    ensure_dir(RECEIPT_DIR, 0, gid, 0o750)
    return gid


def lock():
    if fcntl is None:
        raise RuntimeError("fcntl locking is required")
    ensure_control()
    descriptor = os.open(LOCK_PATH, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    os.fchown(descriptor, 0, 0)
    os.fchmod(descriptor, 0o600)
    fcntl.flock(descriptor, fcntl.LOCK_EX)
    return descriptor


def unlock(descriptor):
    fcntl.flock(descriptor, fcntl.LOCK_UN)
    os.close(descriptor)


def boot_id():
    with open(BOOT_ID_PATH, "r", encoding="ascii") as handle:
        value = handle.read().strip()
    if not BOOT_RE.fullmatch(value):
        raise RuntimeError("invalid kernel boot id")
    return value


def validate_token(value):
    if not isinstance(value, str) or not TOKEN_RE.fullmatch(value):
        raise ValueError("invalid request token")
    return value


def validate_attempt(value):
    if not isinstance(value, str) or not ATTEMPT_RE.fullmatch(value):
        raise ValueError("invalid attempt id")
    return value


def validate_status(phase, disposition=None, portal_suffix=None, error_code=None):
    if phase not in PHASES:
        raise ValueError("invalid setup phase")
    if phase == "starting":
        if disposition not in DISPOSITIONS or portal_suffix is not None or error_code is not None:
            raise ValueError("invalid starting setup status")
    elif phase == "portal_ready":
        if disposition is not None or error_code is not None or not isinstance(portal_suffix, str) or not re.fullmatch(r"[0-9a-f]{4}", portal_suffix):
            raise ValueError("invalid portal_ready setup status")
    elif phase in {"finalizing", "succeeded"}:
        if disposition is not None or portal_suffix is not None or error_code is not None:
            raise ValueError("invalid completion setup status")
    elif phase == "failed":
        if disposition is not None or portal_suffix is not None or error_code not in FAILURE_CODES:
            raise ValueError("invalid failed setup status")
    elif phase == "timed_out":
        if disposition is not None or portal_suffix is not None or error_code != "unavailable":
            raise ValueError("invalid timed_out setup status")
    elif phase == "unsupported":
        if disposition is not None or portal_suffix is not None or error_code != "unsupported":
            raise ValueError("invalid unsupported setup status")


def make_status(phase, disposition=None, portal_suffix=None, error_code=None):
    validate_status(phase, disposition, portal_suffix, error_code)
    status = {"type": "setup_portal_status", "phase": phase}
    if disposition is not None:
        status["disposition"] = disposition
    if portal_suffix is not None:
        status["portalSuffix"] = portal_suffix
    status["rebootRequired"] = False
    if error_code is not None:
        status["errorCode"] = error_code
    return status


def atomic_write(path, content, mode, gid, replace=True):
    directory = os.path.dirname(path)
    metadata = os.lstat(directory)
    if not stat.S_ISDIR(metadata.st_mode) or metadata.st_uid != 0 or stat.S_IMODE(metadata.st_mode) not in (0o700, 0o750):
        raise RuntimeError("unsafe setup write directory")
    if os.path.lexists(path):
        existing = os.lstat(path)
        if not stat.S_ISREG(existing.st_mode) or existing.st_uid != 0 or existing.st_nlink != 1 or not replace:
            raise RuntimeError("unsafe setup destination")
    temporary = os.path.join(directory, f".{os.path.basename(path)}.{secrets.token_hex(8)}.tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        os.fchown(descriptor, 0, gid)
        os.fchmod(descriptor, mode)
        with os.fdopen(descriptor, "wb") as handle:
            descriptor = -1
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chown(path, 0, gid)
        os.chmod(path, mode)
        final = os.lstat(path)
        if not stat.S_ISREG(final.st_mode) or final.st_uid != 0 or final.st_gid != gid or final.st_nlink != 1 or stat.S_IMODE(final.st_mode) != mode:
            raise RuntimeError("unsafe setup output")
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def read_json(path, mode, gid):
    metadata = os.lstat(path)
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_gid != gid or metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) != mode:
        raise RuntimeError("unsafe setup state")
    with open(path, "r", encoding="utf-8") as handle:
        return json.load(handle)


def validate_active(record):
    if not isinstance(record, dict) or set(record) != ACTIVE_FIELDS:
        raise ValueError("invalid active setup state")
    if record["schema"] != SCHEMA or record["bootId"] != boot_id():
        raise ValueError("stale active setup state")
    validate_attempt(record["attemptId"])
    if record["requestToken"] is not None:
        validate_token(record["requestToken"])
    if not isinstance(record["sequence"], int) or record["sequence"] < 1 or not isinstance(record["reentry"], bool) or not isinstance(record["priorSetupComplete"], bool):
        raise ValueError("invalid active setup state")
    if not isinstance(record["startedMonotonic"], (int, float)) or not isinstance(record["deadlineMonotonic"], (int, float)) or record["deadlineMonotonic"] <= record["startedMonotonic"]:
        raise ValueError("invalid active setup deadline")
    if not isinstance(record["servicePid"], int) or record["servicePid"] < 0 or (record["serviceStartTicks"] is not None and not isinstance(record["serviceStartTicks"], int)):
        raise ValueError("invalid active setup process")
    claim = record["claimPath"]
    if claim is not None and (not isinstance(claim, str) or not claim.startswith(CONTROL_DIR + "/claims/")):
        raise ValueError("invalid active claim")


def read_active():
    try:
        record = read_json(ACTIVE_PATH, 0o600, 0)
    except FileNotFoundError:
        return None
    try:
        validate_active(record)
    except (ValueError, RuntimeError):
        remove_claim(record if isinstance(record, dict) else None)
        remove_active_file()
        return None
    return record


def remove_active_file():
    try:
        metadata = os.lstat(ACTIVE_PATH)
        if stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0 and metadata.st_nlink == 1 and stat.S_IMODE(metadata.st_mode) == 0o600:
            os.unlink(ACTIVE_PATH)
    except FileNotFoundError:
        pass


def process_ticks(pid):
    try:
        with open(f"/proc/{pid}/stat", "r", encoding="ascii") as handle:
            return int(handle.read().rsplit(")", 1)[1].split()[19])
    except (FileNotFoundError, IndexError, ValueError, OSError):
        return None


def active_process(record):
    if record["servicePid"] == 0:
        return True
    if record["serviceStartTicks"] is None or process_ticks(record["servicePid"]) != record["serviceStartTicks"]:
        return False
    try:
        os.kill(record["servicePid"], 0)
    except OSError:
        return False
    return True


def active_valid(record):
    return record is not None and time.monotonic() < record["deadlineMonotonic"] and active_process(record)


def unit_state():
    try:
        result = subprocess.run(["systemctl", "show", "--property=ActiveState", "--value", SETUP_UNIT], stdin=subprocess.DEVNULL, capture_output=True, text=True, check=False)
    except OSError:
        return "unknown"
    if result.returncode != 0:
        return "unknown"
    state = result.stdout.strip()
    return state if state in {"active", "activating", "inactive", "deactivating", "failed"} else "unknown"


def inspect_unit_state():
    state = "unknown"
    for _ in range(3):
        state = unit_state()
        if state != "unknown":
            return state
        time.sleep(0.05)
    return state


def next_sequence():
    try:
        with open(SEQUENCE_PATH, "r", encoding="ascii") as handle:
            sequence = int(handle.read())
    except FileNotFoundError:
        sequence = 0
    if not 0 <= sequence < 2**63 - 1:
        raise RuntimeError("invalid setup sequence")
    sequence += 1
    atomic_write(SEQUENCE_PATH, f"{sequence}\n".encode("ascii"), 0o600, 0)
    return sequence


def marker_exists():
    return os.path.lexists(MARKER_PATH)


def write_active(record):
    atomic_write(ACTIVE_PATH, (json.dumps(record, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"), 0o600, 0)


def write_current(record, phase, disposition=None, portal_suffix=None, error_code=None):
    sequence = next_sequence()
    status = make_status(phase, disposition, portal_suffix, error_code)
    payload = {"schema": SCHEMA, "bootId": boot_id(), "attemptId": validate_attempt(record["attemptId"]), "sequence": sequence, "status": status}
    gid = ensure_public()
    atomic_write(os.path.join(PUBLIC_DIR, "current.json"), (json.dumps(payload, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"), 0o640, gid)
    record["sequence"] = sequence
    write_active(record)
    return sequence, status


def write_receipt(request_token, record, status, sequence, replace=True):
    request_token = validate_token(request_token)
    if sequence != record["sequence"]:
        raise ValueError("receipt sequence does not match current status")
    if set(status) - {"type", "phase", "disposition", "portalSuffix", "rebootRequired", "errorCode"}:
        raise ValueError("invalid setup receipt status")
    gid = ensure_public()
    payload = {"schema": SCHEMA, "bootId": boot_id(), "attemptId": validate_attempt(record["attemptId"]), "sequence": sequence, "status": status}
    path = os.path.join(RECEIPT_DIR, f"{request_token}.json")
    atomic_write(path, (json.dumps(payload, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8"), 0o640, gid, replace=replace)


def read_receipt(request_token):
    path = os.path.join(RECEIPT_DIR, f"{validate_token(request_token)}.json")
    try:
        payload = read_json(path, 0o640, group_id())
    except FileNotFoundError:
        return None
    if set(payload) != {"schema", "bootId", "attemptId", "sequence", "status"} or payload["schema"] != SCHEMA or payload["bootId"] != boot_id() or not isinstance(payload["sequence"], int):
        raise ValueError("invalid setup receipt")
    validate_attempt(payload["attemptId"])
    status = payload["status"]
    if not isinstance(status, dict) or status.get("type") != "setup_portal_status":
        raise ValueError("invalid setup receipt status")
    validate_status(status.get("phase"), status.get("disposition"), status.get("portalSuffix"), status.get("errorCode"))
    expected = {"type", "phase", "rebootRequired"}
    if status["phase"] == "starting":
        expected.add("disposition")
    elif status["phase"] == "portal_ready":
        expected.add("portalSuffix")
    elif status["phase"] in {"failed", "timed_out", "unsupported"}:
        expected.add("errorCode")
    if set(status) != expected or any(value is None for value in status.values()):
        raise ValueError("invalid setup receipt status")
    return payload


def remove_claim(record):
    claim = record.get("claimPath") if record else None
    if not claim:
        return
    try:
        metadata = os.lstat(claim)
        if stat.S_ISREG(metadata.st_mode) and metadata.st_uid == 0 and metadata.st_nlink == 1:
            os.unlink(claim)
    except FileNotFoundError:
        pass


def remove_active(record):
    remove_claim(record)
    remove_active_file()


def new_record(request_token, reentry, claim_path):
    now = time.monotonic()
    return {"schema": SCHEMA, "bootId": boot_id(), "attemptId": secrets.token_hex(16), "requestToken": request_token, "sequence": 0, "reentry": reentry, "priorSetupComplete": marker_exists(), "startedMonotonic": now, "deadlineMonotonic": now + 1800.0, "servicePid": 0, "serviceStartTicks": None, "claimPath": claim_path}


def finish_failed(record, error_code, extra_token=None):
    sequence, status = write_current(record, "failed", error_code=error_code)
    if record["requestToken"] is not None:
        write_receipt(record["requestToken"], record, status, sequence)
    if extra_token is not None and extra_token != record["requestToken"]:
        write_receipt(extra_token, record, status, sequence)
    remove_active(record)
    return sequence


def start_or_attach(request_token, reentry, claim_path):
    validate_token(request_token)
    if not isinstance(claim_path, str) or not claim_path.startswith(CONTROL_DIR + "/claims/"):
        raise ValueError("invalid setup claim")
    descriptor = lock()
    try:
        existing = read_active()
        state = inspect_unit_state()
        if existing is not None and active_valid(existing):
            if state in {"active", "activating"}:
                existing_receipt = read_receipt(request_token)
                if existing_receipt is None:
                    status = make_status("starting", disposition="already_running")
                    write_receipt(request_token, existing, status, existing["sequence"])
                print(json.dumps({"decision": "attached", "attemptId": existing["attemptId"]}, separators=(",", ":")))
                return 0
            if state == "inactive" and existing["servicePid"] == 0:
                finish_failed(existing, "operation_failed")
                existing = None
                state = "inactive"
            elif state == "unknown":
                finish_failed(existing, "unavailable", request_token)
                print(json.dumps({"decision": "failed", "attemptId": existing["attemptId"]}, separators=(",", ":")))
                return 0
            elif state in {"failed", "deactivating"}:
                finish_failed(existing, "operation_failed", request_token)
                print(json.dumps({"decision": "failed", "attemptId": existing["attemptId"]}, separators=(",", ":")))
                return 0
            elif existing is not None:
                finish_failed(existing, "operation_failed")
                existing = None
                state = "inactive"
        elif existing is not None:
            remove_active(existing)
        receipt = read_receipt(request_token) if existing is None else None
        if receipt is not None:
            print(json.dumps({"decision": "replayed", "attemptId": receipt["attemptId"]}, separators=(",", ":")))
            return 0
        if state == "inactive":
            record = new_record(request_token, reentry, claim_path)
            write_active(record)
            sequence, status = write_current(record, "starting", disposition="accepted")
            write_receipt(request_token, record, status, sequence)
            print(json.dumps({"decision": "new", "attemptId": record["attemptId"]}, separators=(",", ":")))
            return 0
        record = new_record(request_token, reentry, claim_path)
        write_active(record)
        error_code = "unavailable" if state == "unknown" else "operation_failed"
        finish_failed(record, error_code)
        print(json.dumps({"decision": "failed", "attemptId": record["attemptId"]}, separators=(",", ":")))
        return 0
    finally:
        unlock(descriptor)


def start_failed(request_token):
    validate_token(request_token)
    descriptor = lock()
    try:
        record = read_active()
        if record is not None and record["requestToken"] == request_token:
            finish_failed(record, "operation_failed")
        return 0
    finally:
        unlock(descriptor)


def ensure_firstboot():
    descriptor = lock()
    try:
        existing = read_active()
        if active_valid(existing):
            return 0
        if existing is not None:
            remove_active(existing)
        record = new_record(None, False, None)
        write_active(record)
        write_current(record, "starting", disposition="accepted")
        return 0
    finally:
        unlock(descriptor)


def update_status(command, phase, error_code, portal_suffix):
    descriptor = lock()
    try:
        record = read_active()
        if record is None or not active_valid(record):
            return 1
        write_current(record, phase, error_code=error_code or None, portal_suffix=portal_suffix)
        return 0
    finally:
        unlock(descriptor)


def terminal_status(phase, error_code, portal_suffix):
    descriptor = lock()
    try:
        record = read_active()
        if record is None:
            return 0
        if phase in {"failed", "timed_out"} or active_valid(record):
            sequence, status = write_current(record, phase, error_code=error_code or None, portal_suffix=portal_suffix)
            if record["requestToken"] is not None:
                write_receipt(record["requestToken"], record, status, sequence)
            remove_active(record)
            return 0
        return 1
    finally:
        unlock(descriptor)


def stop():
    return terminal_status("failed", "operation_failed", None)


if __name__ == "__main__":
    cli_spec = importlib.util.spec_from_file_location("octessera_setup_status_cli", os.path.join(os.path.dirname(__file__), "setup-status-cli.py"))
    cli = importlib.util.module_from_spec(cli_spec)
    cli_spec.loader.exec_module(cli)
    raise SystemExit(cli.main(sys.modules[__name__], sys.argv))
