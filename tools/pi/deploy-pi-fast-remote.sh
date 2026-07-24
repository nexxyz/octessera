#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 11 ]; then
    echo "fast deployment helper received an invalid argument count" >&2
    exit 2
fi

INSTALL_DIR="$1"
SERVICE="$2"
PROFILE="$3"
EXPECTED_SCHEMA="$4"
EXPECTED_BINARY="$5"
EXPECTED_ARCH="$6"
EXPECTED_RUNTIME_ARCH="$7"
EXPECTED_CARGO_FEATURE="$8"
CANDIDATE_PATH="$9"
CANDIDATE_METADATA_PATH="${10}"
ALLOW_SERVICE_FAILURE="${11}"

if [ "$PROFILE" != raspberry-pi-zero-2w ] || [ "$EXPECTED_BINARY" != octessera-pi ] || [ "$EXPECTED_SCHEMA" != 1 ] || [ "$EXPECTED_ARCH" != aarch64-unknown-linux-gnu ] || [ "$EXPECTED_RUNTIME_ARCH" != aarch64 ] || [ "$EXPECTED_CARGO_FEATURE" != hardware-raspberry-pi-zero-2w ]; then
    echo "fast deployment expected metadata is not the canonical Raspberry profile" >&2
    exit 1
fi

RELEASES_DIR="$INSTALL_DIR/releases"
CURRENT_LINK="$INSTALL_DIR/current"
CURRENT_BINARY="$INSTALL_DIR/current/octessera-pi"
STATE_PATH="$INSTALL_DIR/update-state.json"
PROFILE_METADATA_PATH="$INSTALL_DIR/board-profile.json"
BIN_LINK="${OCTESSERA_FAST_DEPLOY_BIN_LINK:-/usr/local/bin/octessera-pi}"
PROC_ROOT="${OCTESSERA_FAST_DEPLOY_PROC_ROOT:-/proc}"
WORK_DIR="$(mktemp -d /tmp/octessera-fast-deploy.XXXXXX)"
STAGING_RELEASE=""
PRIOR_CURRENT_TARGET=""
PRIOR_CURRENT_PATH=""
PRIOR_CURRENT_VERSION=""
PRIOR_BINARY_TARGET=""
PRIOR_STATE_PRESENT=0
PRIOR_PROFILE_METADATA_PRESENT=0
STATE_CHANGED=0
PROFILE_METADATA_CHANGED=0
ACTIVATED=0
SERVICE_ATTEMPTED=0
ROLLING_BACK=0

atomic_link() {
    local target="$1"
    local link="$2"
    local temporary="${link}.fast-deploy-$$"
    rm -f -- "$temporary"
    ln -s -- "$target" "$temporary"
    mv -Tf -- "$temporary" "$link"
}

restore_file() {
    local source="$1"
    local destination="$2"
    local temporary="${destination}.fast-deploy-$$"
    cp -p -- "$source" "$temporary"
    mv -Tf -- "$temporary" "$destination"
}

verify_service() {
    local expected_binary="$1"
    local main_pid
    local executable
    systemctl --no-pager --lines=8 status "$SERVICE" || return 1
    systemctl is-active --quiet "$SERVICE" || return 1
    sleep 1
    systemctl is-active --quiet "$SERVICE" || return 1
    main_pid="$(systemctl show "$SERVICE" -p MainPID --value)"
    case "$main_pid" in
        ''|*[!0-9]*|0) return 1 ;;
    esac
    executable="$(readlink -f -- "$PROC_ROOT/$main_pid/exe")"
    [ "$executable" = "$expected_binary" ]
}

cleanup() {
    rm -f -- "$CANDIDATE_PATH" "$CANDIDATE_METADATA_PATH"
    if [ -n "$STAGING_RELEASE" ] && { [ -e "$STAGING_RELEASE" ] || [ -L "$STAGING_RELEASE" ]; }; then
        chmod -R u+w -- "$STAGING_RELEASE" 2>/dev/null || true
        rm -rf -- "$STAGING_RELEASE"
    fi
    rm -rf -- "$WORK_DIR"
}

rollback() {
    local original_status="$1"
    local rollback_status=0
    ROLLING_BACK=1
    set +e
    if [ "$SERVICE_ATTEMPTED" -eq 1 ]; then
        systemctl stop "$SERVICE" || rollback_status=1
    fi
    if [ "$ACTIVATED" -eq 1 ]; then
        atomic_link "$PRIOR_CURRENT_TARGET" "$CURRENT_LINK" || rollback_status=1
        atomic_link "$PRIOR_BINARY_TARGET" "$BIN_LINK" || rollback_status=1
    fi
    if [ "$STATE_CHANGED" -eq 1 ]; then
        if [ "$PRIOR_STATE_PRESENT" -eq 1 ]; then
            restore_file "$WORK_DIR/state.backup" "$STATE_PATH" || rollback_status=1
        else
            rm -f -- "$STATE_PATH" || rollback_status=1
        fi
    fi
    if [ "$PROFILE_METADATA_CHANGED" -eq 1 ]; then
        if [ "$PRIOR_PROFILE_METADATA_PRESENT" -eq 1 ]; then
            restore_file "$WORK_DIR/profile-metadata.backup" "$PROFILE_METADATA_PATH" || rollback_status=1
        else
            rm -f -- "$PROFILE_METADATA_PATH" || rollback_status=1
        fi
    fi
    if [ "$SERVICE_ATTEMPTED" -eq 1 ]; then
        systemctl restart "$SERVICE" || rollback_status=1
        verify_service "$PRIOR_CURRENT_PATH" || rollback_status=1
    fi
    if [ -n "$STAGING_RELEASE" ] && { [ -e "$STAGING_RELEASE" ] || [ -L "$STAGING_RELEASE" ]; }; then
        chmod -R u+w -- "$STAGING_RELEASE" 2>/dev/null || rollback_status=1
        rm -rf -- "$STAGING_RELEASE" || rollback_status=1
    fi
    rm -f -- "$CANDIDATE_PATH" "$CANDIDATE_METADATA_PATH"
    rm -rf -- "$WORK_DIR"
    if [ "$rollback_status" -ne 0 ]; then
        echo "Fast deployment failed and prior runtime restoration could not be verified." >&2
        exit 1
    fi
    if [ "$SERVICE_ATTEMPTED" -eq 1 ] && [ "${ALLOW_SERVICE_FAILURE:-0}" -eq 1 ]; then
        echo "Fast deployment rolled back after a service failure; -AllowServiceFailure was specified." >&2
        exit 0
    fi
    exit "$original_status"
}

on_exit() {
    local status="$?"
    if [ "$ROLLING_BACK" -eq 1 ]; then
        exit "$status"
    fi
    if [ "$status" -ne 0 ]; then
        rollback "$status"
    fi
    cleanup
}

trap on_exit EXIT

if [ -e "$INSTALL_DIR/update-transaction.json" ] || [ -e "$INSTALL_DIR/update-state.json.next" ]; then
    echo "Refusing fast deployment while an updater transaction is pending; use recovery or rollback first." >&2
    exit 75
fi
if [ ! -L "$CURRENT_LINK" ] || [ ! -L "$BIN_LINK" ]; then
    echo "Managed current and binary links are required for rollback-safe deployment." >&2
    exit 1
fi

PRIOR_CURRENT_TARGET="$(readlink -- "$CURRENT_LINK")"
PRIOR_CURRENT_PATH="$(readlink -f -- "$CURRENT_LINK")/octessera-pi"
PRIOR_CURRENT_VERSION="$(basename -- "$(readlink -f -- "$CURRENT_LINK")")"
PRIOR_BINARY_TARGET="$(readlink -- "$BIN_LINK")"
if [ ! -f "$PRIOR_CURRENT_PATH" ] || [ ! -x "$PRIOR_CURRENT_PATH" ]; then
    echo "Prior current binary is not executable; refusing deployment." >&2
    exit 1
fi
if [ -e "$STATE_PATH" ] || [ -L "$STATE_PATH" ]; then
    if [ -L "$STATE_PATH" ] || [ ! -f "$STATE_PATH" ]; then
        echo "Updater state is not a regular file; refusing deployment." >&2
        exit 1
    fi
    cp -p -- "$STATE_PATH" "$WORK_DIR/state.backup"
    PRIOR_STATE_PRESENT=1
fi
if [ -e "$PROFILE_METADATA_PATH" ] || [ -L "$PROFILE_METADATA_PATH" ]; then
    if [ -L "$PROFILE_METADATA_PATH" ] || [ ! -f "$PROFILE_METADATA_PATH" ]; then
        echo "Board metadata is not a regular file; refusing deployment." >&2
        exit 1
    fi
    cp -p -- "$PROFILE_METADATA_PATH" "$WORK_DIR/profile-metadata.backup"
    PRIOR_PROFILE_METADATA_PRESENT=1
fi

install -d -m 0755 "$RELEASES_DIR"
CANDIDATE_SUFFIX="$(date -u +%s)$$"
CANDIDATE_VERSION="0.0.$CANDIDATE_SUFFIX"
while [ -e "$RELEASES_DIR/$CANDIDATE_VERSION" ] || [ -L "$RELEASES_DIR/$CANDIDATE_VERSION" ]; do
    sleep 1
    CANDIDATE_SUFFIX="$(date -u +%s)$$"
    CANDIDATE_VERSION="0.0.$CANDIDATE_SUFFIX"
done
STAGING_RELEASE="$RELEASES_DIR/$CANDIDATE_VERSION"
install -d -m 0755 "$STAGING_RELEASE"
install -m 0755 "$CANDIDATE_PATH" "$STAGING_RELEASE/octessera-pi"

"$STAGING_RELEASE/octessera-pi" --print-build-metadata > "$WORK_DIR/runtime-metadata.json"
python3 - "$WORK_DIR/runtime-metadata.json" "$EXPECTED_SCHEMA" "$PROFILE" "$EXPECTED_BINARY" "$EXPECTED_RUNTIME_ARCH" <<'PY'
import json
import sys
from pathlib import Path


def unique_object(pairs):
    result = {}
    for name, value in pairs:
        if name in result:
            raise ValueError(f"duplicate JSON property after decoding: {name}")
        result[name] = value
    return result


payload = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"), object_pairs_hook=unique_object)
expected_schema, expected_profile, expected_binary, expected_arch = sys.argv[2:]
if type(payload) is not dict:
    raise ValueError("candidate metadata is not an object")
if set(payload) != {"schema_version", "board_profile", "binary", "arch", "package_version"}:
    raise ValueError("candidate metadata fields are invalid")
if type(payload["schema_version"]) is not int or str(payload["schema_version"]) != expected_schema:
    raise ValueError("candidate metadata schema_version is invalid")
if payload["board_profile"] != expected_profile:
    raise ValueError("candidate metadata board_profile is invalid")
if payload["binary"] != expected_binary:
    raise ValueError("candidate metadata binary is invalid")
if payload["arch"] != expected_arch:
    raise ValueError("candidate metadata arch is invalid")
if type(payload["package_version"]) is not str or not payload["package_version"].strip():
    raise ValueError("candidate metadata package_version is invalid")
PY

printf '{"schema_version":2,"updater_protocol":2,"candidate_health_protocol":1,"tag":"v%s","version":"%s","board_profile":"%s","arch":"aarch64-unknown-linux-gnu","binary":"octessera-pi","platforms":["%s","linux-aarch64-device"]}\n' "$CANDIDATE_VERSION" "$CANDIDATE_VERSION" "$PROFILE" "$PROFILE" > "$STAGING_RELEASE/update-manifest.json"
chmod 0555 "$STAGING_RELEASE"
chmod 0555 "$STAGING_RELEASE/octessera-pi"
chmod 0444 "$STAGING_RELEASE/update-manifest.json"

if [ "$PRIOR_STATE_PRESENT" -eq 1 ]; then
    python3 - "$WORK_DIR/state.next" "$CANDIDATE_VERSION" "$PRIOR_CURRENT_VERSION" "$STAGING_RELEASE/update-manifest.json" <<'PY'
import json
import sys
from datetime import datetime, timezone


output, current, previous, manifest_path = sys.argv[1:]
with open(manifest_path, encoding="utf-8") as handle:
    manifest = json.load(handle)
payload = {
    "schema_version": 2,
    "phase": "committed",
    "current": current,
    "previous": previous if previous.count(".") == 2 and all(part.isdigit() for part in previous.split(".")) else None,
    "updated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
    "release": manifest,
    "asset": None,
}
with open(output, "w", encoding="utf-8") as handle:
    json.dump(payload, handle, indent=2)
    handle.write("\n")
PY
    chmod 0644 "$WORK_DIR/state.next"
fi

PROFILE_METADATA_CHANGED=1
install -m 0644 "$CANDIDATE_METADATA_PATH" "$PROFILE_METADATA_PATH"
ACTIVATED=1
atomic_link "$STAGING_RELEASE" "$CURRENT_LINK"
atomic_link "$CURRENT_BINARY" "$BIN_LINK"
if [ "$PRIOR_STATE_PRESENT" -eq 1 ]; then
    STATE_CHANGED=1
    mv -Tf -- "$WORK_DIR/state.next" "$STATE_PATH"
fi
SERVICE_ATTEMPTED=1
systemctl restart "$SERVICE"
verify_service "$STAGING_RELEASE/octessera-pi"
STAGING_RELEASE=""
echo "Fast deployment activated $CANDIDATE_VERSION."
