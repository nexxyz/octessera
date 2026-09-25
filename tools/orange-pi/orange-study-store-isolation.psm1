Set-StrictMode -Version Latest

function Replace-OrangeAwakeAnchor {
  param([string]$Text, [string]$Anchor, [string]$Replacement)
  $index = $Text.IndexOf($Anchor, [StringComparison]::Ordinal)
  if ($index -lt 0 -or $index -ne $Text.LastIndexOf($Anchor, [StringComparison]::Ordinal)) {
    throw "Orange AWAKE study payload anchor is missing or duplicated: $Anchor"
  }
  return $Text.Replace($Anchor, $Replacement)
}

function Get-OrangeStudyStorePython {
  return @'
import hashlib
import json
import os
import re
import stat
import sys

mode, source, clone, evidence = sys.argv[1:]
names = ("default.json", "recovery-save.json")
if not re.fullmatch(r"octessera-study-[0-9a-f]{32}\.service", os.path.basename(clone)):
    raise ValueError("AWAKE clone path must match the transient unit")

def regular_bytes(path, required):
    try:
        metadata = os.lstat(path)
    except FileNotFoundError:
        if required:
            raise
        return None
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError("AWAKE source must be a regular file: " + path)
    with open(path, "rb") as handle:
        return handle.read()

if mode == "prepare":
    if not stat.S_ISDIR(os.lstat(source).st_mode):
        raise ValueError("AWAKE source store is not a directory")
    if not stat.S_ISDIR(os.lstat(os.path.dirname(clone)).st_mode):
        raise ValueError("AWAKE clone parent is not a directory")
    original = {name: regular_bytes(os.path.join(source, name), name == "default.json") for name in names}
    payload = json.loads(original["default.json"])
    runtime = payload.get("runtimeConfig") if isinstance(payload, dict) else None
    keys = ("dimTimerSeconds", "screenSleepSeconds")
    if not isinstance(runtime, dict) or any(type(runtime.get(key)) is not int for key in keys):
        raise ValueError("AWAKE default must contain both numeric timers")
    timers = {key: runtime[key] for key in keys}
    runtime.update({key: 0 for key in keys})
    awake = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8") + b"\n"
    print(json.dumps({"scenario": "AWAKE", "clone_path": clone, "original_timers": timers,
                      "original_sha256": {name: hashlib.sha256(data).hexdigest() if data is not None else None
                                          for name, data in original.items()}}), flush=True)
    os.mkdir(clone, 0o700)
    for name in names:
        data = awake if name == "default.json" else original[name]
        if data is not None:
            fd = os.open(os.path.join(clone, name), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, "wb") as handle:
                handle.write(data)
elif mode == "verify":
    with open(evidence, encoding="utf-8") as handle:
        recorded = json.load(handle)
    if recorded["scenario"] != "AWAKE" or recorded["clone_path"] != clone or set(recorded["original_sha256"]) != set(names):
        raise ValueError("AWAKE original hash evidence is invalid")
    for name in names:
        data = regular_bytes(os.path.join(source, name), False)
        digest = hashlib.sha256(data).hexdigest() if data is not None else None
        if digest != recorded["original_sha256"][name]:
            raise ValueError("AWAKE original store changed: " + name)
    print("AWAKE original store hashes unchanged")
else:
    raise ValueError("AWAKE study store action is invalid")
'@
}

function New-OrangeAwakeStudyPayloadBundle {
  param([Parameter(Mandatory)]$Bundle)
  $storeFunctions = @'
study_parent=/var/lib/octessera/study-stores
study_store="$study_parent/$unit"
study_store_python() {
  sudo -n python3 - "$@" <<'PY'
__PYTHON__
PY
}
verify_study_store() {
  study_store_python verify /var/lib/octessera/presets "$study_store" "$root/awake-store-evidence.json" > "$root/awake-store-verify.txt" 2>&1
}
prepare_study_store() {
  if [ ! -e "$study_parent" ]; then
    sudo -n install -d -o root -g octessera-runtime -m 0750 "$study_parent"
    printf 'yes\n' > "$root/awake-parent-created.txt"
  fi
  study_store_python prepare /var/lib/octessera/presets "$study_store" /dev/null > "$root/awake-store-evidence.json"
  sudo -n chown -R octessera-runtime:octessera-runtime "$study_store"
  verify_study_store
}
cleanup_study_store() {
  [ -d "$study_store" ] && [ ! -L "$study_store" ] || return 1
  sudo -n rm -rf -- "$study_store" || return 1
  if [ -r "$root/awake-parent-created.txt" ] && grep -Fxq yes "$root/awake-parent-created.txt"; then
    sudo -n rmdir -- "$study_parent" 2>/dev/null || true
  fi
}
'@.Replace('__PYTHON__', (Get-OrangeStudyStorePython))
  $study = $Bundle.Study
  $study = Replace-OrangeAwakeAnchor $study 'candidate_status=0' "$storeFunctions`ncandidate_status=0"
  $study = Replace-OrangeAwakeAnchor $study 'sudo -n systemctl stop "$service"' "prepare_study_store`nsudo -n systemctl stop `"`$service`""
  $study = Replace-OrangeAwakeAnchor $study "'store_dir=/var/lib/octessera/presets'" '"store_dir=$study_store"'
  $study = Replace-OrangeAwakeAnchor $study '--setenv=OCTESSERA_PI_STORE_DIR=/var/lib/octessera/presets' '--setenv=OCTESSERA_PI_STORE_DIR="$study_store" --setenv=OCTESSERA_PI_TIMING_KEEP_AWAKE=1'
  $study = Replace-OrangeAwakeAnchor $study '      ready=1' @'
      if ! sudo -n systemctl show "$unit" --property=Environment --value | tr ' ' '\n' | grep -Fxq "OCTESSERA_PI_STORE_DIR=$study_store"; then
        candidate_status=5
        break
      fi
      ready=1
'@
  $study = Replace-OrangeAwakeAnchor $study '    candidate_status=2' '    [ "$candidate_status" -ne 0 ] || candidate_status=2'
  $verifyBeforeRestore = @'
  if ! verify_study_store; then
    restore_status=1
    return
  fi
'@
  $study = Replace-OrangeAwakeAnchor $study '  if ! timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"' "$verifyBeforeRestore`n  if ! timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start `"`$service`""
  $cleanupAfterRestore = @'
  if [ "$exit_status" -eq 0 ] && [ "$restore_status" -eq 0 ] && [ "$cleanup_status" -eq 0 ]; then
    if sudo -n systemctl is-active --quiet "$unit"; then
      cleanup_status=1
    else
      cleanup_study_store || cleanup_status=1
    fi
  fi
'@
  $study = Replace-OrangeAwakeAnchor $study '  restore_service' "  restore_service`n$cleanupAfterRestore"
  $study = Replace-OrangeAwakeAnchor $study 'exit "$candidate_status"' "printf 'scenario=AWAKE\n' >> `"`$root/study-result.txt`"`nexit `"`$candidate_status`""
  $Bundle.Study = $study

  $cleanup = $Bundle.Cleanup
  $cleanup = Replace-OrangeAwakeAnchor $cleanup 'state_file="$root/service-initial-state.txt"' "$storeFunctions`nstate_file=`"`$root/service-initial-state.txt`""
  $cleanupGuard = @'
  if [ -e "$root/awake-store-evidence.json" ] || [ -e "$study_store" ]; then
    verify_study_store || exit 72
  fi
'@
  $cleanup = Replace-OrangeAwakeAnchor $cleanup '  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start "$service"' "$cleanupGuard`n  timeout --signal=TERM --kill-after=2 15s sudo -n systemctl start `"`$service`""
  $Bundle.Cleanup = $cleanup
  return $Bundle
}

Export-ModuleMember -Function Get-OrangeStudyStorePython, New-OrangeAwakeStudyPayloadBundle
