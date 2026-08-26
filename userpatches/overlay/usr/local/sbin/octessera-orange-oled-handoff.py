#!/usr/bin/env python3
import errno
import fcntl
import json
import os
import pwd
import re
import stat
import time


HANDOFF_ROOT = "/run/octessera-boot"
SCHEMA = 1
DIRECTORY_MODE = 0o750
LOCK_MODE = 0o600
STATUS_MODE = 0o640
STOP_MODE = 0o600
FATAL_MODE = 0o600
MAX_STATUS_BYTES = 4096
MAX_STOP_BYTES = 1024
MAX_FATAL_BYTES = 256
UTILITY_LOCK_TIMEOUT_SECONDS = 0.25
SHUTDOWN_LOCK_TIMEOUT_SECONDS = 4.0
PHASES = {"animating", "release_requested", "released", "native_owned", "first_menu_rendered", "failed"}
FATAL_CODES = {"trellis_unavailable", "neokey_unavailable", "controls_unavailable", "audio_unavailable", "oled_unavailable", "startup_failed"}
BOOT_ID_RE = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
REQUEST_ID_RE = re.compile(r"^[0-9a-f]{32}$")
RUNTIME_USER = "octessera-runtime"


def current_boot_id():
    value = open("/proc/sys/kernel/random/boot_id", encoding="ascii").read().strip()
    if not BOOT_ID_RE.fullmatch(value):
        raise RuntimeError("invalid current boot id")
    return value


def runtime_identity():
    try:
        account = pwd.getpwnam(RUNTIME_USER)
    except KeyError as error:
        raise RuntimeError(f"missing fixed OLED runtime account: {RUNTIME_USER}") from error
    return account.pw_uid, account.pw_gid


def _valid_metadata(metadata, mode, regular=True):
    expected_type = stat.S_IFREG if regular else stat.S_IFDIR
    runtime_uid, runtime_gid = runtime_identity()
    return stat.S_IFMT(metadata.st_mode) == expected_type and metadata.st_uid == runtime_uid and metadata.st_gid == runtime_gid and stat.S_IMODE(metadata.st_mode) == mode and (not regular or metadata.st_nlink == 1)


def _valid_root(path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC | os.O_NOFOLLOW)
    metadata = os.fstat(descriptor)
    if not _valid_metadata(metadata, DIRECTORY_MODE, regular=False):
        os.close(descriptor)
        raise RuntimeError("invalid OLED handoff directory")
    return descriptor


def _read_file(path, mode, maximum):
    descriptor = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        metadata = os.fstat(descriptor)
        if not _valid_metadata(metadata, mode) or metadata.st_size > maximum:
            raise RuntimeError("invalid OLED handoff file")
        data = os.read(descriptor, maximum + 1)
        if len(data) > maximum:
            raise RuntimeError("oversized OLED handoff file")
        return data
    finally:
        os.close(descriptor)


def _strict_json(data, name):
    try:
        value = json.loads(data.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise RuntimeError(f"malformed OLED {name}") from error
    if not isinstance(value, dict):
        raise RuntimeError(f"OLED {name} must be an object")
    return value


def _status(phase, boot_id, pid, cycle_count, request_id=None):
    if phase not in PHASES or not isinstance(boot_id, str) or not BOOT_ID_RE.fullmatch(boot_id) or not isinstance(pid, int) or not 0 < pid <= 0xFFFFFFFF or not isinstance(cycle_count, int) or cycle_count < 0:
        raise ValueError("invalid OLED status")
    needs_request = phase != "animating"
    if needs_request != (request_id is not None) or (request_id is not None and (not isinstance(request_id, str) or not REQUEST_ID_RE.fullmatch(request_id))):
        raise ValueError("invalid OLED status request")
    value = {"schema": SCHEMA, "phase": phase, "bootId": boot_id, "pid": pid, "cycleCount": cycle_count}
    if request_id is not None:
        value["requestId"] = request_id
    return value


def parse_status(value):
    phase = value.get("phase")
    if not isinstance(phase, str) or phase not in PHASES:
        raise RuntimeError("invalid OLED status phase")
    expected = {"schema", "phase", "bootId", "pid", "cycleCount"}
    if phase != "animating":
        expected.add("requestId")
    if set(value) != expected or value.get("schema") != SCHEMA:
        raise RuntimeError("OLED status has unknown or missing keys")
    return _status(phase, value["bootId"], value["pid"], value["cycleCount"], value.get("requestId"))


def parse_stop(value):
    if set(value) != {"schema", "bootId", "pid", "requestId"} or value.get("schema") != SCHEMA or not isinstance(value.get("bootId"), str) or not BOOT_ID_RE.fullmatch(value["bootId"]) or not isinstance(value.get("pid"), int) or not 0 < value["pid"] <= 0xFFFFFFFF or not isinstance(value.get("requestId"), str) or not REQUEST_ID_RE.fullmatch(value["requestId"]):
        raise RuntimeError("invalid OLED stop request")
    return value


def parse_fatal(value):
    if set(value) != {"schema", "bootId", "code"} or type(value.get("schema")) is not int or value.get("schema") != SCHEMA or not isinstance(value.get("bootId"), str) or not BOOT_ID_RE.fullmatch(value["bootId"]) or not isinstance(value.get("code"), str) or value["code"] not in FATAL_CODES:
        raise RuntimeError("invalid OLED fatal status")
    return value


def _terminal_status(status, stop, boot_id):
    return (
        status is not None
        and status["bootId"] == boot_id
        and status["phase"] == "first_menu_rendered"
        and stop is not None
        and stop["bootId"] == boot_id
        and stop["requestId"] == status["requestId"]
    )


class Handoff:
    def __init__(self, root, directory, boot_id, lock):
        self.root = root
        self.directory = directory
        self.boot_id = boot_id
        self.lock = lock
        self.lock_held = True
        self.request_id = None
        self.cycle_count = 0

    @classmethod
    def open(cls, create_lock, timeout_seconds=None):
        root = HANDOFF_ROOT
        directory = _valid_root(root)
        boot_id = current_boot_id()
        lock_path = os.path.join(root, "oled.lock")
        try:
            lock = os.open(lock_path, os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW, LOCK_MODE)
        except FileNotFoundError:
            if not create_lock:
                os.close(directory)
                raise RuntimeError("OLED handoff lock is missing")
            lock = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW, LOCK_MODE)
            os.fchmod(lock, LOCK_MODE)
        metadata = os.fstat(lock)
        if not _valid_metadata(metadata, LOCK_MODE):
            os.close(lock)
            os.close(directory)
            raise RuntimeError("invalid OLED handoff lock")
        if timeout_seconds is None:
            fcntl.flock(lock, fcntl.LOCK_EX)
        else:
            deadline = time.monotonic() + timeout_seconds
            while True:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        os.close(lock)
                        os.close(directory)
                        raise TimeoutError("OLED utility lock timed out")
                    time.sleep(min(0.01, remaining))
        handoff = cls(root, directory, boot_id, lock)
        handoff.validate_entries()
        return handoff

    @classmethod
    def peek_terminal(cls):
        root = HANDOFF_ROOT
        directory = _valid_root(root)
        try:
            boot_id = current_boot_id()
            probe = cls(root, directory, boot_id, None)
            probe.validate_entries()
            status_before = probe._read_status()
            stop_before = probe._read_stop()
            status_after = probe._read_status()
            stop_after = probe._read_stop()
            return (
                status_before == status_after
                and stop_before == stop_after
                and _terminal_status(status_before, stop_before, boot_id)
            )
        finally:
            os.close(directory)

    @classmethod
    def utility_lock(cls, timeout_seconds=UTILITY_LOCK_TIMEOUT_SECONDS):
        return cls.open(False, timeout_seconds)

    def validate_entries(self):
        allowed = {"oled.lock", "status.json", "stop.request", "fatal.json"}
        for name in os.listdir(self.root):
            if name in allowed:
                continue
            if name.startswith(".status.json.tmp-") and REQUEST_ID_RE.fullmatch(name[-32:]):
                continue
            if name.startswith(".stop.request.tmp-") and REQUEST_ID_RE.fullmatch(name[-32:]):
                continue
            if name.startswith(".fatal.json.tmp-") and REQUEST_ID_RE.fullmatch(name[-32:]):
                continue
            raise RuntimeError(f"unknown OLED handoff entry: {name}")

    def _read_status(self):
        try:
            return parse_status(_strict_json(_read_file(os.path.join(self.root, "status.json"), STATUS_MODE, MAX_STATUS_BYTES), "status.json"))
        except FileNotFoundError:
            return None

    def _read_stop(self):
        try:
            return parse_stop(_strict_json(_read_file(os.path.join(self.root, "stop.request"), STOP_MODE, MAX_STOP_BYTES), "stop.request"))
        except FileNotFoundError:
            return None

    def _read_fatal(self):
        value = self._read_fatal_value()
        if value is None:
            return None
        return parse_fatal(value)

    def _read_fatal_value(self):
        try:
            return _strict_json(_read_file(os.path.join(self.root, "fatal.json"), FATAL_MODE, MAX_FATAL_BYTES), "fatal.json")
        except FileNotFoundError:
            return None

    def startup_fatal_code(self):
        try:
            value = self._read_fatal_value()
        except (OSError, RuntimeError):
            return "startup_failed"
        if value is None:
            return None
        boot_id = value.get("bootId")
        if isinstance(boot_id, str) and BOOT_ID_RE.fullmatch(boot_id) and boot_id != self.boot_id:
            return None
        try:
            fatal = parse_fatal(value)
        except RuntimeError:
            return "startup_failed"
        return fatal["code"]

    def _write(self, name, value, mode, no_clobber=False):
        data = (json.dumps(value, separators=(",", ":"), sort_keys=True) + "\n").encode("utf-8")
        temporary = os.path.join(self.root, f".{name}.tmp-{os.urandom(16).hex()}")
        descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_CLOEXEC | os.O_NOFOLLOW, mode)
        try:
            os.fchmod(descriptor, mode)
            view = memoryview(data)
            while view:
                view = view[os.write(descriptor, view):]
            os.fsync(descriptor)
        finally:
            os.close(descriptor)
        try:
            if no_clobber:
                try:
                    os.link(temporary, os.path.join(self.root, name), follow_symlinks=False)
                except FileExistsError:
                    return False
                os.unlink(temporary)
            else:
                target = os.path.join(self.root, name)
                if os.path.lexists(target):
                    metadata = os.lstat(target)
                    if not _valid_metadata(metadata, mode):
                        raise RuntimeError(f"invalid existing OLED {name}")
                os.replace(temporary, target)
            return True
        finally:
            try:
                os.unlink(temporary)
            except FileNotFoundError:
                pass

    def _write_status(self, phase, request_id=None):
        value = _status(phase, self.boot_id, os.getpid(), self.cycle_count, request_id)
        self._write("status.json", value, STATUS_MODE)

    def _create_stop(self):
        existing = self._read_stop()
        if self.request_id is not None:
            request_id = self.request_id
        elif existing is not None:
            if existing["bootId"] != self.boot_id:
                raise RuntimeError("OLED stop request belongs to another boot")
            request_id = existing["requestId"]
        else:
            request_id = os.urandom(16).hex()
        value = {"schema": SCHEMA, "bootId": self.boot_id, "pid": os.getpid(), "requestId": request_id}
        if self._write("stop.request", value, STOP_MODE, no_clobber=True):
            return request_id
        existing = self._read_stop()
        if existing is None or existing["bootId"] != self.boot_id:
            raise RuntimeError("OLED stop request disappeared")
        return existing["requestId"]

    def start(self):
        previous = self._read_status()
        stop = self._read_stop()
        try:
            fatal = self._read_fatal()
        except (OSError, RuntimeError):
            fatal = None
        if previous is not None and previous["bootId"] == self.boot_id:
            raise RuntimeError("OLED handoff already exists for this boot")
        if stop is not None and stop["bootId"] == self.boot_id:
            raise RuntimeError("OLED stop request already exists for this boot")
        if previous is not None and stop is not None and stop["bootId"] != previous["bootId"]:
            raise RuntimeError("OLED handoff entries belong to different boots")
        if previous is not None or stop is not None:
            for name in ("status.json", "stop.request"):
                try:
                    os.unlink(os.path.join(self.root, name))
                except FileNotFoundError:
                    pass
            if fatal is not None and fatal["bootId"] != self.boot_id:
                try:
                    os.unlink(os.path.join(self.root, "fatal.json"))
                except FileNotFoundError:
                    pass
        self._write_status("animating")

    def stop_requested(self):
        stop = self._read_stop()
        if stop is None:
            return False
        if stop["bootId"] != self.boot_id:
            raise RuntimeError("OLED stop request belongs to another boot")
        if self.request_id is not None and self.request_id != stop["requestId"]:
            raise RuntimeError("OLED stop request changed during animation")
        self.request_id = stop["requestId"]
        self._write_status("release_requested", self.request_id)
        return True

    def publish_cycle(self):
        if self.request_id is not None:
            raise RuntimeError("OLED release was already requested")
        self.cycle_count += 1
        self._write_status("animating")

    def mark_failed(self):
        request_id = self._create_stop()
        self.request_id = request_id
        self._write_status("failed", request_id)

    def release(self):
        if self.request_id is None:
            raise RuntimeError("OLED release requires a stop request")
        self._write_status("released", self.request_id)

    def release_existing(self):
        if self.request_id is None:
            raise RuntimeError("OLED existing release requires a stop request")
        status = self._read_status()
        stop = self._read_stop()
        if status is None or status["bootId"] != self.boot_id or status["requestId"] != self.request_id or status["phase"] not in {"release_requested", "released", "native_owned", "failed"}:
            raise RuntimeError("OLED existing status changed before retry release")
        if stop is None or stop["bootId"] != self.boot_id or stop["requestId"] != self.request_id:
            raise RuntimeError("OLED existing stop request changed before retry release")
        if status["phase"] != "release_requested":
            self._write_status("release_requested", self.request_id)
        self.release()

    def unlock_preserving(self):
        if self.lock_held:
            fcntl.flock(self.lock, fcntl.LOCK_UN)
            self.lock_held = False

    def reacquire_nonblocking(self, request_id=None):
        if self.lock_held:
            raise RuntimeError("OLED handoff lock is already held")
        expected_request_id = request_id if request_id is not None else self.request_id
        if expected_request_id is None or not REQUEST_ID_RE.fullmatch(expected_request_id):
            raise RuntimeError("OLED existing handoff request is missing")
        try:
            fcntl.flock(self.lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return None
        except OSError as error:
            if error.errno in {errno.EACCES, errno.EAGAIN}:
                return None
            raise
        self.lock_held = True
        try:
            status = self._read_status()
            stop = self._read_stop()
            if status is None or status["bootId"] != self.boot_id or status["requestId"] != expected_request_id or status["phase"] not in {"release_requested", "released", "native_owned", "first_menu_rendered", "failed"}:
                raise RuntimeError("OLED existing status changed during reacquire")
            if stop is None or stop["bootId"] != self.boot_id or stop["requestId"] != expected_request_id:
                raise RuntimeError("OLED existing stop request changed during reacquire")
            self.request_id = expected_request_id
            return status
        except Exception:
            self.unlock_preserving()
            raise

    def close(self):
        try:
            if self.lock_held:
                fcntl.flock(self.lock, fcntl.LOCK_UN)
                self.lock_held = False
        finally:
            os.close(self.lock)
            os.close(self.directory)
