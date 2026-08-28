#!/usr/bin/env python3
import base64
import binascii
import os
import re
import stat
import subprocess
import tempfile
import time


PROFILE_PATH = "/etc/octessera/setup-profile"
MARKER_PATH = "/var/lib/octessera/setup-complete"
SSH_POLICY_PATH = "/etc/ssh/sshd_config.d/10-octessera-setup.conf"
ALLOWED_FIELDS = frozenset(("sshMode", "sshPublicKey", "sshPassword", "sshPasswordConfirm", "hostname", "wifiCountry"))
KEY_TYPES = frozenset(("ssh-ed25519", "ssh-rsa", "ecdsa-sha2-nistp256", "ecdsa-sha2-nistp384", "ecdsa-sha2-nistp521"))
KEY_LINE = re.compile(r"^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(?:256|384|521)) ([A-Za-z0-9+/]+={0,2})(?: ([ -~]{1,256}))?$")
HOSTNAME_LABEL = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$")
RASPBERRY_PROFILE = "r" "aspberry-pi-zero-2w"
PROFILES = {
    "orange-pi-zero-2w": {"user": "octessera", "request_owner": "octessera-runtime", "status_group": "octessera-runtime", "ssh_units": ("ssh.service", "ssh.socket", "sshd.service", "sshd.socket")},
    RASPBERRY_PROFILE: {"user": "pi", "request_owner": "pi", "status_group": "pi", "ssh_units": ("ssh.service", "ssh.socket")},
}


def _has_control(value):
    return any(ord(character) < 32 or ord(character) == 127 for character in value)


def valid_hostname(value):
    return isinstance(value, str) and (value == "" or (len(value) <= 253 and all(
        len(label) <= 63 and bool(HOSTNAME_LABEL.fullmatch(label)) for label in value.split(".")
    )))


def valid_country(value):
    return isinstance(value, str) and (value == "" or bool(re.fullmatch(r"[A-Z]{2}", value)))


def valid_public_key(value):
    if not isinstance(value, str) or len(value) > 4096 or "\n" in value or "\r" in value or _has_control(value):
        return False
    match = KEY_LINE.fullmatch(value)
    if match is None:
        return False
    key_type, encoded = match.group(1), match.group(2)
    if key_type not in KEY_TYPES or len(encoded) % 4 != 0:
        return False
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except (ValueError, binascii.Error):
        return False
    if not 32 <= len(decoded) <= 4096:
        return False
    type_length = int.from_bytes(decoded[:4], "big")
    return type_length == len(key_type) and decoded[4 : 4 + type_length] == key_type.encode("ascii")


def valid_password(value):
    return isinstance(value, str) and 8 <= len(value) <= 128 and bool(value.strip()) and not _has_control(value)


def validate_country_payload(data):
    if not isinstance(data, dict) or frozenset(data) != frozenset(("wifiCountry",)):
        raise ValueError("invalid country")
    value = data["wifiCountry"]
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z]{2}", value):
        raise ValueError("invalid country")
    return value.upper()


def validate_stage(data):
    if not isinstance(data, dict) or frozenset(data) != ALLOWED_FIELDS:
        raise ValueError("invalid stage")
    if any(not isinstance(data[field], str) for field in ALLOWED_FIELDS):
        raise ValueError("invalid stage")
    mode = data["sshMode"]
    hostname = data["hostname"].strip()
    country = data["wifiCountry"].strip().upper()
    if mode not in ("none", "key", "password") or _has_control(data["hostname"]):
        raise ValueError("invalid stage")
    if not valid_hostname(hostname) or not valid_country(country):
        raise ValueError("invalid stage")
    result = {"sshMode": mode, "hostname": hostname, "country": country}
    if mode == "key":
        if not valid_public_key(data["sshPublicKey"]) or data["sshPassword"] or data["sshPasswordConfirm"]:
            raise ValueError("invalid stage")
        result["sshKey"] = data["sshPublicKey"]
    elif mode == "password":
        password = data["sshPassword"]
        if data["sshPublicKey"] or data["sshPasswordConfirm"] != password or not valid_password(password):
            raise ValueError("invalid stage")
        result["password"] = password
    elif any(data[field] for field in ("sshPublicKey", "sshPassword", "sshPasswordConfirm")):
        raise ValueError("invalid stage")
    return result


def load_profile(path=PROFILE_PATH):
    metadata = os.lstat(path)
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_uid != 0 or metadata.st_nlink != 1 or stat.S_IMODE(metadata.st_mode) != 0o644:
        raise OSError("unsafe setup profile")
    with open(path, "rb") as handle:
        value = handle.read().decode("ascii")
    profile = value[:-1] if value.endswith("\n") else value
    if value != f"{profile}\n" or profile not in PROFILES:
        raise ValueError("invalid setup profile")
    return {"profile": profile, **PROFILES[profile]}


def run(args, input_text=None, timeout=None):
    subprocess.run(args, input=input_text, text=True, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=timeout)


def _remaining(deadline, clock):
    if deadline is None:
        return None
    remaining = deadline - clock()
    if remaining <= 0:
        raise subprocess.TimeoutExpired([], 0)
    return remaining


def _write_atomic(path, content, mode, owner=0, group=0):
    directory = os.path.dirname(path)
    os.makedirs(directory, mode=0o755, exist_ok=True)
    descriptor, temporary = tempfile.mkstemp(prefix=f".{os.path.basename(path)}.", suffix=".tmp", dir=directory)
    try:
        os.fchmod(descriptor, mode)
        os.fchown(descriptor, owner, group)
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            descriptor = -1
            handle.write(content)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        os.chmod(path, mode)
        os.chown(path, owner, group)
    finally:
        if descriptor >= 0:
            os.close(descriptor)
        try:
            os.unlink(temporary)
        except FileNotFoundError:
            pass


def configure_key(key, profile):
    import pwd

    account = pwd.getpwnam(profile["user"])
    ssh_dir = os.path.join(account.pw_dir, ".ssh")
    if os.path.lexists(ssh_dir):
        metadata = os.lstat(ssh_dir)
        if not stat.S_ISDIR(metadata.st_mode):
            raise OSError("unsafe ssh directory")
    else:
        os.mkdir(ssh_dir, 0o700)
    os.chown(ssh_dir, account.pw_uid, account.pw_gid)
    os.chmod(ssh_dir, 0o700)
    _write_atomic(os.path.join(ssh_dir, "authorized_keys"), key + "\n", 0o600, account.pw_uid, account.pw_gid)


def remove_key(profile):
    import pwd

    account = pwd.getpwnam(profile["user"])
    path = os.path.join(account.pw_dir, ".ssh", "authorized_keys")
    try:
        metadata = os.lstat(path)
        if stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1:
            os.unlink(path)
    except FileNotFoundError:
        pass


def set_password_auth(enabled, profile):
    value = "yes" if enabled else "no"
    _write_atomic(SSH_POLICY_PATH, f"PermitRootLogin no\nPasswordAuthentication {value}\nAllowUsers {profile['user']}\n", 0o644)


def apply_country(country):
    if country:
        run(["iw", "reg", "set", country])


def persist_country(country):
    if not country:
        return
    _write_atomic("/etc/modprobe.d/octessera-regdom.conf", f"options cfg80211 ieee80211_regdom={country}\n", 0o644)
    if os.path.exists("/etc/default/crda"):
        _write_atomic("/etc/default/crda", f"REGDOMAIN={country}\n", 0o644)


def finalize(data, profile, deadline=None, clock=time.monotonic):
    invoke = lambda args, input_text=None: run(args, input_text, timeout=_remaining(deadline, clock))
    data = validate_stage({
        "sshMode": data["sshMode"],
        "sshPublicKey": data.get("sshKey", ""),
        "sshPassword": data.get("password", ""),
        "sshPasswordConfirm": data.get("password", ""),
        "hostname": data["hostname"],
        "wifiCountry": data["country"],
    })
    if data["hostname"]:
        invoke(["hostnamectl", "set-hostname", data["hostname"]])
    persist_country(data["country"])
    mode = data["sshMode"]
    if mode == "key":
        configure_key(data["sshKey"], profile)
        set_password_auth(False, profile)
    elif mode == "password":
        remove_key(profile)
        invoke(["chpasswd"], f"{profile['user']}:{data['password']}\n")
        set_password_auth(True, profile)
    else:
        remove_key(profile)
        invoke(["passwd", "-l", profile["user"]])
        set_password_auth(False, profile)
        invoke(["systemctl", "disable", "--now", "ssh.service"])
        invoke(["systemctl", "disable", "--now", "ssh.socket"])
        for unit in profile["ssh_units"]:
            invoke(["systemctl", "mask", unit])
    if mode in ("key", "password"):
        invoke(["ssh-keygen", "-A"])
        for unit in profile["ssh_units"]:
            invoke(["systemctl", "unmask", unit])
        invoke(["systemctl", "enable", "--now", "ssh.service"])
        invoke(["systemctl", "reload", "ssh.service"])
    _write_atomic(MARKER_PATH, "complete\n", 0o644)
