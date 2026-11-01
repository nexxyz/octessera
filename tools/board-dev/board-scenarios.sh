#!/usr/bin/env bash
# On-device scenario suite for one board, for what desktop e2e, golden tests, board-check and the
# timing smoke cannot see: cold boot to playable, save/restore through real menu input, an
# Audio+Video recording, OLED sleep/wake and brightness on the SPI bus, and a 10-minute mixed-load
# soak. Run it before image builds and releases.
#
#   tools/board-dev/board-scenarios.sh rpi pi@<PI_HOST> [scenario ...]
#   tools/board-dev/board-scenarios.sh opi octessera@<ORANGE_HOST> [scenario ...]
# Scenarios: boot save_restore recording oled soak (default: all). boot really reboots the board and
# only reads the live service; the others run detached on a cloned store, with no SSH until they end.
# Results and evidence land in target/board-scenarios/<board>-<stamp>/, PASS/FAIL lines in summary.txt.
set -euo pipefail
board=${1:?board: rpi or opi} target=${2:?target: user@host}
shift 2
[[ "$board" == rpi || "$board" == opi ]] || { echo "board must be rpi or opi" >&2; exit 2; }
scenarios=("$@")
[ ${#scenarios[@]} -gt 0 ] || scenarios=(boot save_restore recording oled soak)
soak_seconds=${OCTESSERA_SCENARIO_SOAK_SECONDS:-1500}
cd "$(git rev-parse --show-toplevel)"
remote() { powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/board-dev/with-board-ssh.ps1 "$1" -Board "$board" -Target "$target" "${@:2}"; }
local_path() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
out="target/board-scenarios/$board-$(date +%Y%m%d-%H%M%S)"
root="/tmp/octessera-scenarios-$(date +%s)"
mkdir -p "$out"
upload() {
  remote ssh "mkdir -m 755 $root"
  for file in board-scenarios-payload.sh board-keys.py; do
    remote scp "$(local_path "tools/board-dev/$file")" "$target:$root/$file" >/dev/null
  done
}
fetch() {
  for file in "$@"; do
    remote scp "$target:$root/$file" "$(local_path "$out/$file")" >/dev/null 2>&1 || echo "not fetched: $file" >> "$out/fetch.log"
  done
  remote ssh "rm -rf $root"
}

run=()
wait=30
for scenario in "${scenarios[@]}"; do
  case $scenario in
    boot) ;;
    save_restore) run+=("$scenario"); wait=$((wait + 150)) ;;
    recording) run+=("$scenario"); wait=$((wait + 75)) ;;
    oled) run+=("$scenario"); wait=$((wait + 120)) ;;
    soak) run+=("$scenario"); wait=$((wait + soak_seconds + 60)) ;;
    *) echo "unknown scenario: $scenario" >&2; exit 2 ;;
  esac
done

if [[ " ${scenarios[*]} " == *" boot "* ]]; then
  live=/home/pi/presets
  [ "$board" = opi ] && live=/var/lib/octessera/presets
  remote ssh "sudo -n sha256sum $live/system.json $live/default.patch.json $live/recovery-save.json" > "$out/live-hashes-before.log"
  remote ssh "sudo -n systemctl reboot" || true
  echo "rebooting $board; no SSH for 90 s"
  sleep 90
  for _ in $(seq 1 30); do remote ssh "systemctl is-active --quiet octessera.service" && break; sleep 10; done
  sleep 10
  upload
  remote ssh "bash $root/board-scenarios-payload.sh $board $root boot"
  fetch boot-host.log ready.json boot-journal.log boot-service.json failed-units.log live-hashes.log
fi

if [ ${#run[@]} -gt 0 ]; then
  upload
  remote ssh "setsid nohup taskset -c 0 bash $root/board-scenarios-payload.sh $board $root run $soak_seconds ${run[*]} > $root/host.log 2>&1 < /dev/null &"
  echo "running ${run[*]} on $board; no SSH for ${wait}s"
  sleep "$wait"
  for _ in $(seq 1 30); do remote ssh "test -f $root/done" && break; sleep 20; done
  remote ssh "test -f $root/done" || { echo "payload still running; $root left on the board" >&2; exit 1; }
  fetch host.log marks.log journal.log save-before-system.json save-before-patch.json \
    save-edited-system.json save-edited-patch.json save-restarted-system.json save-restarted-patch.json \
    save-runtime-system.json save-runtime-patch.json oled-before-system.json oled-trace.txt soak-proc.log recording.avi
fi

python tools/board-dev/board-scenarios-check.py "$board" "$out" "${scenarios[@]}" | tee "$out/summary.txt"
! grep -q '^FAIL' "$out/summary.txt"
