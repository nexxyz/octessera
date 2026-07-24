#!/usr/bin/env python3
import os
import math
import time
from pathlib import Path
from typing import TypedDict

from updater_protocol import BINARY, CANDIDATE_HEALTH_PROTOCOL, MARKER_SCHEMA, MAX_JSON_BYTES, read_json, same_path


class ServiceSnapshot(TypedDict):
    main_pid: int
    invocation_id: str
    n_restarts: int
    active_state: str
    sub_state: str


def _number(updater, name: str, default: float, maximum: float) -> float:
    try:
        value = float(os.environ.get(name, default))
    except ValueError as exc:
        raise updater.error(f"Invalid updater timing value: {name}") from exc
    if not math.isfinite(value):
        raise updater.error(f"Invalid updater timing value: {name}")
    return max(0.01, min(value, maximum))


def marker(updater, pid: int, invocation: str, expected: dict) -> None:
    error = updater.error
    if expected.get("candidate_health_protocol") != CANDIDATE_HEALTH_PROTOCOL:
        raise error("Candidate health protocol declaration is invalid")
    if not updater.health_path.exists():
        raise error("Candidate readiness marker did not arrive")
    payload = read_json(updater.health_path, 64 * 1024)
    if not isinstance(payload, dict) or payload.get("schema_version") != MARKER_SCHEMA:
        raise error("Candidate readiness marker schema is invalid")
    marker_invocation = payload.get("systemd_invocation_id", payload.get("invocation_id"))
    ready_ms = payload.get("ready_at_unix_ms", payload.get("ready_at_ms"))
    if payload.get("pid") != pid or marker_invocation != invocation or payload.get("package_version") != expected["version"] or payload.get("board_profile") != updater.profile:
        raise error("Candidate readiness identity does not match the running service")
    if not isinstance(ready_ms, int) or ready_ms > int(time.time() * 1000) + 60000:
        raise error("Candidate readiness time is invalid")


def process_exe(updater, pid: int) -> Path:
    error = updater.error
    proc_root = Path(os.environ.get("OCTESSERA_UPDATE_PROC_ROOT", "/proc"))
    try:
        raw = os.readlink(proc_root / str(pid) / "exe")
    except OSError as exc:
        raise error("Candidate process executable is unavailable") from exc
    if raw.endswith(" (deleted)"):
        raw = raw[:-10]
    return Path(raw).resolve(strict=False)


def systemctl_properties(updater) -> dict[str, str]:
    output = updater.systemctl_call([
        "show", updater.service_name, "-p", "MainPID", "-p", "InvocationID",
        "-p", "NRestarts", "-p", "ActiveState", "-p", "SubState",
    ])
    properties = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
    for name in ("MainPID", "InvocationID", "NRestarts", "ActiveState", "SubState"):
        if name not in properties:
            raise updater.error(f"systemd snapshot is missing {name}")
    return properties


def _integer(updater, properties: dict[str, str], name: str) -> int:
    try:
        value = int(properties[name] or "0")
    except ValueError as exc:
        raise updater.error(f"systemd property {name} is not an integer") from exc
    if value < 0:
        raise updater.error(f"systemd property {name} is negative")
    return value


def service_snapshot(updater) -> ServiceSnapshot:
    properties = systemctl_properties(updater)
    return {
        "main_pid": _integer(updater, properties, "MainPID"),
        "invocation_id": properties["InvocationID"],
        "n_restarts": _integer(updater, properties, "NRestarts"),
        "active_state": properties["ActiveState"],
        "sub_state": properties["SubState"],
    }


def require_recovery_active(updater) -> None:
    output = updater.systemctl_call([
        "show", updater.recovery_service_name, "-p", "ActiveState", "-p", "SubState",
    ])
    properties = dict(line.split("=", 1) for line in output.splitlines() if "=" in line)
    if properties.get("ActiveState") != "active" or properties.get("SubState") not in ("exited", "running"):
        raise updater.error("Updater recovery service is not active")


def verify_service_inactive(updater) -> None:
    state = service_snapshot(updater)
    if state["main_pid"] != 0 or state["active_state"] not in ("inactive", "failed"):
        raise updater.error("Boot recovery requires octessera.service to be inactive")


def _restore(updater, payload: dict, stop_service: bool, original: Exception) -> None:
    try:
        updater.restore_transaction(payload, stop_service=stop_service)
    except Exception as rollback_error:
        raise updater.error(f"Updater rollback failed: {rollback_error}") from original
    if isinstance(original, updater.error):
        raise original
    raise updater.error(f"Updater guard failed: {original}") from original


def guard_transaction(updater) -> None:
    error = updater.error
    if not updater.transaction_path.exists():
        return
    updater.require_recovery_active()
    payload = None
    activation_attempted = False
    try:
        try:
            payload = updater.load_transaction()
        except error:
            updater.recover_pending()
            return
        if payload.get("candidate_source") == "downloaded" and not updater.profile:
            updater.profile = payload.get("board_profile", "")
        if payload["phase"] != "validating":
            raise error("Prepared transaction was not switched before guard start")
        candidate = Path(payload["candidate"]["path"])
        expected = payload["candidate"]["manifest"]
        current, _ = updater.current_link()
        if current != candidate.name:
            raise error("Current link does not identify the candidate")
        before = service_snapshot(updater)
        updater.mark_activation_attempted(payload)
        activation_attempted = True
        try:
            updater.systemctl_call(["restart", updater.service_name])
        except Exception as exc:
            raise error(f"Candidate service restart failed: {exc}") from exc

        timeout = _number(updater, "OCTESSERA_UPDATE_READINESS_TIMEOUT", 60, 300)
        stability = _number(updater, "OCTESSERA_UPDATE_STABILITY_WINDOW", 15, 120)
        interval = _number(updater, "OCTESSERA_UPDATE_POLL_SECONDS", 0.25, 2)
        require_health = payload.get("candidate_source") == "downloaded"
        identity = None
        started = time.monotonic()
        while time.monotonic() - started < timeout:
            state = service_snapshot(updater)
            pid = state["main_pid"]
            invocation = state["invocation_id"]
            restarts = state["n_restarts"]
            if state["active_state"] == "failed":
                raise error("Candidate service failed")
            if pid > 0 and invocation and state["active_state"] == "active":
                if before["main_pid"] != 0 and (pid, invocation) == (before["main_pid"], before["invocation_id"]):
                    time.sleep(interval)
                    continue
                if before["n_restarts"] != restarts:
                    raise error("Candidate service restart count changed")
                if same_path(process_exe(updater, pid), (candidate / BINARY).resolve(strict=False)) is False:
                    raise error("Candidate process executable identity mismatch")
                identity = (pid, invocation, restarts)
                if require_health:
                    marker(updater, pid, invocation, expected)
                break
            time.sleep(interval)
        else:
            raise error("Candidate readiness timeout")

        stable_started = time.monotonic()
        while time.monotonic() - stable_started < stability:
            state = service_snapshot(updater)
            current_identity = (state["main_pid"], state["invocation_id"], state["n_restarts"])
            if current_identity != identity or state["active_state"] != "active":
                raise error("Candidate PID, invocation, or restart count changed")
            if not same_path(process_exe(updater, current_identity[0]), (candidate / BINARY).resolve(strict=False)):
                raise error("Candidate executable changed during stability window")
            if require_health:
                marker(updater, current_identity[0], current_identity[1], expected)
            time.sleep(interval)

        asset_path = candidate / "update-asset.json"
        asset = read_json(asset_path, MAX_JSON_BYTES) if asset_path.exists() else None
        updater.write_committed_state(candidate.name, payload["fallback"]["current"], expected, asset)
        updater.transaction_path.unlink(missing_ok=True)
        updater.health_path.unlink(missing_ok=True)
    except Exception as exc:
        if payload is None:
            try:
                updater.recover_legacy(force=True)
                updater.transaction_path.unlink(missing_ok=True)
                updater.health_path.unlink(missing_ok=True)
            except Exception as rollback_error:
                raise error(f"Updater recovery failed: {rollback_error}") from exc
            raise error(f"Updater guard failed: {exc}") from exc
        _restore(updater, payload, activation_attempted, exc)


def stop_service_verified(updater) -> None:
    before = systemctl_properties(updater)
    if before["MainPID"] == "0" and before["ActiveState"] in ("inactive", "failed"):
        return
    updater.systemctl_call(["stop", updater.service_name])
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        state = systemctl_properties(updater)
        if state["MainPID"] == "0" and state["ActiveState"] in ("inactive", "failed"):
            return
        time.sleep(0.1)
    raise updater.error("Candidate service did not stop")


def start_service_verified(updater, expected_version: str) -> None:
    updater.systemctl_call(["start", updater.service_name])
    expected = (updater.releases / expected_version / BINARY).resolve(strict=False)
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        state = systemctl_properties(updater)
        if state["ActiveState"] == "active" and int(state["MainPID"] or "0") > 0:
            try:
                executable = Path(os.readlink(Path(os.environ.get("OCTESSERA_UPDATE_PROC_ROOT", "/proc")) / state["MainPID"] / "exe")).resolve(strict=False)
            except OSError as exc:
                raise updater.error("Fallback process executable is unavailable") from exc
            if not same_path(executable, expected):
                raise updater.error("Fallback service executable identity mismatch")
            return
        time.sleep(0.1)
    raise updater.error("Fallback service did not become active")
