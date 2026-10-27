#!/usr/bin/env bash
# Runs on the board (started detached by board-timing-smoke.sh): plays the default patch from a
# cloned store with the OLED awake, then restores the installed service and the original store.
set -Eeuo pipefail
board=$1 live_seconds=$2 root=$3
shift 3
extra_env=()
for setting in "$@"; do extra_env+=("--setenv=$setting"); done
run_id=${root##*-}
store_id=$(head -c 16 /dev/urandom | od -An -tx1 | tr -d " \n")
service=octessera.service
unit="octessera-smoke-$run_id.service"
store_dir="/var/lib/octessera/study-stores/octessera-study-$store_id.service"
health="/run/octessera/smoke-ready-$run_id.json"
if [ "$board" = rpi ]; then
  runtime_user=pi live_store=/home/pi/presets samples_dir=/home/pi/samples
  working_directory="$(systemctl show "$service" -p WorkingDirectory --value)"
  props=(--property=User=pi --property=Group=pi --property=LimitMEMLOCK=infinity
    --property=WorkingDirectory="$working_directory" "--property=SupplementaryGroups=tty input"
    "--property=AmbientCapabilities=CAP_SYS_NICE CAP_SYS_TTY_CONFIG"
    "--property=CapabilityBoundingSet=CAP_SYS_NICE CAP_SETUID CAP_SETGID CAP_SYS_TTY_CONFIG"
    --property=NoNewPrivileges=no --property=TTYPath=/dev/tty1 --property=TTYReset=yes
    --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=raspberry-pi-zero-2w)
else
  runtime_user=octessera-runtime live_store=/var/lib/octessera/presets
  samples_dir=/var/lib/octessera/samples
  props=(--property=User=octessera-runtime --property=Group=octessera-runtime --property=Nice=-10
    --property=LimitRTPRIO=70 --property=LimitMEMLOCK=infinity --property=NoNewPrivileges=yes
    --property=ProtectSystem=strict --property=ProtectHome=yes --property=PrivateTmp=no
    --property=RuntimeDirectory=octessera --property=RuntimeDirectoryMode=0755
    "--property=ReadWritePaths=/var/lib/octessera /run/octessera /run/octessera-boot"
    --setenv=OCTESSERA_EXPECTED_BOARD_PROFILE=orange-pi-zero-2w)
fi
hashes() { sudo -n sha256sum "$live_store/system.json" "$live_store/default.patch.json" | awk '{print $1}' | tr '\n' ' '; }
original="$(hashes)"
stopped=0
cleanup() {
  set +e
  sudo -n systemctl stop "$unit" >/dev/null 2>&1
  sudo -n journalctl -u "$unit" --no-pager -o short-monotonic 2>&1 | tee "$root/journal.log" >/dev/null
  sudo -n rm -rf -- "$store_dir"
  if [ "$stopped" -eq 1 ]; then sudo -n systemctl start "$service"; fi
  for _ in $(seq 1 30); do [ "$(systemctl is-active "$service")" = active ] && break; sleep 1; done
  printf 'restore service=%s store_unchanged=%s\n' "$(systemctl is-active "$service")" \
    "$([ "$(hashes)" = "$original" ] && echo yes || echo NO)"
  touch "$root/done"
}
trap cleanup EXIT
sudo -n install -d -o root -g root -m 0755 /var/lib/octessera/study-stores
sudo -n install -d -o "$runtime_user" -g "$runtime_user" -m 0750 "$store_dir"
sudo -n cp -a -- "$live_store/." "$store_dir/"
soak=0
for setting in "$@"; do [ "$setting" = OCTESSERA_TIMING_SOAK=1 ] && soak=1; done
sudo -n python3 - "$store_dir/system.json" "$soak" <<'PY'
import json, sys
path = sys.argv[1]
system = json.load(open(path))
config = system["runtimeConfig"]
if sys.argv[2] != "1":
    config["dimTimerSeconds"] = 0
    config["screenSleepSeconds"] = 0
config["autoSaveDefault"] = False
json.dump(system, open(path, "w"), separators=(",", ":"))
PY
sudo -n chown -R "$runtime_user:$runtime_user" "$store_dir"
chmod 0755 "$root" "$root/octessera-pi"
stopped=1
sudo -n systemctl stop "$service"
for _ in $(seq 1 40); do sudo -n sh -c 'ls -l /proc/[0-9]*/fd 2>/dev/null' | grep -q /dev/snd/pcm || break; sleep 0.5; done
sudo -n install -d -o "$runtime_user" -g "$runtime_user" -m 0755 /run/octessera
sudo -n systemd-run --unit="$unit" --property=Restart=no --property=RuntimeMaxSec="$((live_seconds + 60))" \
  --property=TimeoutStopSec=5s "${props[@]}" --setenv=OCTESSERA_OLED_BOOT_HANDOFF=v1 \
  --setenv=OCTESSERA_TIMING_AUTOPLAY=1 --setenv=OCTESSERA_PI_UI_PROFILE=1 \
  --setenv=OCTESSERA_RUNTIME_TIMING_TRACE=1 --setenv="OCTESSERA_PI_STORE_DIR=$store_dir" \
  --setenv="OCTESSERA_PI_SAMPLES_DIR=$samples_dir" --setenv="OCTESSERA_CANDIDATE_HEALTH_PATH=$health" \
  "${extra_env[@]}" "$root/octessera-pi"
for _ in $(seq 1 30); do sudo -n test -f "$health" && break; systemctl is-active --quiet "$unit" || break; sleep 1; done
for _ in $(seq 1 "$live_seconds"); do systemctl is-active --quiet "$unit" || { echo 'candidate exited early'; break; }; sleep 1; done
