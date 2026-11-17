import json
import os
import stat


STATUS_DIR = "/run/octessera-setup-status"
STATUS_PATH = f"{STATUS_DIR}/current.json"
PASSPHRASE_ALPHABET = "23479ACDEFHJKLMNPQRTUVWXYZabcdefghijkmnpqrstuvwxyz"


def portal_status(phase, disposition=None, portal_suffix=None, error_code=None, portal_passphrase=None):
    if phase == "starting":
        if disposition != "accepted" or portal_suffix is not None or portal_passphrase is not None or error_code is not None:
            raise ValueError("invalid starting status")
    elif phase == "portal_ready":
        if disposition is not None or error_code is not None or not isinstance(portal_suffix, str) or len(portal_suffix) != 4 or any(character not in "0123456789abcdef" for character in portal_suffix) or not isinstance(portal_passphrase, str) or len(portal_passphrase) != 9 or portal_passphrase[4] != "-" or any(character not in PASSPHRASE_ALPHABET for character in portal_passphrase[:4] + portal_passphrase[5:]):
            raise ValueError("invalid portal status")
    elif phase in ("finalizing", "succeeded", "stopped"):
        if disposition is not None or portal_suffix is not None or portal_passphrase is not None or error_code is not None:
            raise ValueError("invalid terminal status")
    elif phase == "failed":
        if disposition is not None or portal_suffix is not None or portal_passphrase is not None or error_code not in ("operation_failed", "radio_restart_required"):
            raise ValueError("invalid failure status")
    elif phase == "timed_out":
        if disposition is not None or portal_suffix is not None or portal_passphrase is not None or error_code != "unavailable":
            raise ValueError("invalid timeout status")
    else:
        raise ValueError("invalid setup phase")
    value = {"type": "setup_portal_status", "phase": phase}
    if disposition is not None:
        value["disposition"] = disposition
    if portal_suffix is not None:
        value["portalSuffix"] = portal_suffix
    if portal_passphrase is not None:
        value["portalPassphrase"] = portal_passphrase
    value["rebootRequired"] = False
    if error_code is not None:
        value["errorCode"] = error_code
    return value


def _status_directory(gid):
    if os.path.lexists(STATUS_DIR):
        metadata = os.lstat(STATUS_DIR)
        if not (stat.S_ISDIR(metadata.st_mode) and metadata.st_uid == 0 and metadata.st_gid == gid and stat.S_IMODE(metadata.st_mode) == 0o750):
            raise OSError("unsafe setup status directory")
    else:
        os.mkdir(STATUS_DIR, 0o750)
        os.chown(STATUS_DIR, 0, gid)
        os.chmod(STATUS_DIR, 0o750)


def write_status(status, profile):
    import grp

    gid = grp.getgrnam(profile["status_group"]).gr_gid
    _status_directory(gid)
    if os.path.lexists(STATUS_PATH):
        existing = os.lstat(STATUS_PATH)
        if not stat.S_ISREG(existing.st_mode) or existing.st_uid != 0 or existing.st_nlink != 1:
            raise OSError("unsafe setup status file")
    temporary = f"{STATUS_PATH}.{os.getpid()}.tmp"
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        payload = {"schema": 1, "status": status}
        os.fchown(descriptor, 0, gid)
        os.fchmod(descriptor, 0o640)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            descriptor = -1
            json.dump(payload, handle, separators=(",", ":"), sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, STATUS_PATH)
        os.chown(STATUS_PATH, 0, gid)
        os.chmod(STATUS_PATH, 0o640)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def status_is_terminal(profile):
    try:
        import grp

        metadata = os.lstat(STATUS_PATH)
        gid = grp.getgrnam(profile["status_group"]).gr_gid
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_gid != gid or metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) != 0o640:
            return False
        with open(STATUS_PATH, "r", encoding="utf-8") as handle:
            payload = json.load(handle)
        status = payload["status"]
        return payload.get("schema") == 1 and status.get("type") == "setup_portal_status" and status.get("phase") in {"succeeded", "stopped", "failed", "timed_out"}
    except (AttributeError, KeyError, OSError, TypeError, UnicodeDecodeError, ValueError, json.JSONDecodeError):
        return False
