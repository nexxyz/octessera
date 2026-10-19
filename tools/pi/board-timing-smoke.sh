#!/usr/bin/env bash
# Shared-path timing smoke on one board: plays the default patch with a candidate binary for a
# few minutes (OLED awake), then reports underruns, pulse cadence and runtime lateness.
# The board runs it detached, so there is no SSH traffic during the measurement.
#
#   tools/pi/board-timing-smoke.sh rpi pi@<PI_HOST> target/pi-cross/octessera-pi [seconds]
#   tools/pi/board-timing-smoke.sh opi octessera@<ORANGE_HOST> target/orange-pi-cross/octessera-pi
set -euo pipefail
board=${1:?board: rpi or opi} target=${2:?target: user@host} binary=${3:?candidate binary}
seconds=${4:-600}
case "$board" in
  rpi) transport=tools/pi/with-rpi-ssh.ps1 ;;
  opi) transport=tools/orange-pi/with-opi-ssh.ps1 ;;
  *) echo "board must be rpi or opi" >&2; exit 2 ;;
esac
cd "$(git rev-parse --show-toplevel)"
remote() { powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$transport" "$@"; }
local_path() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
root="/tmp/octessera-smoke-$(date +%s)"
out="target/board-timing-smoke/$board-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"

remote ssh -Target "$target" "mkdir -m 755 $root"
remote scp -Target "$target" "$(local_path "$binary")" "$target:$root/octessera-pi" >/dev/null
remote scp -Target "$target" "$(local_path tools/pi/board-timing-smoke-payload.sh)" "$target:$root/payload.sh" >/dev/null
remote ssh -Target "$target" "chmod +x $root/octessera-pi; setsid nohup taskset -c 2,3 bash $root/payload.sh $board $seconds $root > $root/host.log 2>&1 < /dev/null &"
echo "running on $board for ${seconds}s; results land in $out"
sleep "$((seconds + 45))"
for _ in $(seq 1 30); do remote ssh -Target "$target" "test -f $root/done" && break; sleep 10; done
for file in host.log journal.log; do
  remote scp -Target "$target" "$target:$root/$file" "$(local_path "$out/$file")" >/dev/null
done
remote ssh -Target "$target" "rm -rf $root"

journal="$out/journal.log"
echo "underruns=$(grep -c 'underrun occurred' "$journal" || true)"
grep 'runtime-timing playing' "$journal" | tail -n 1 | grep -o 'dispatches=[0-9]* pulses=[0-9]* max_pulses=[0-9]*\|dispatch=\[[^]]*\]' || true
grep 'pi-ui-profile' "$journal" | tail -n 1 | grep -o 'runtime_late=[^ ]* [^ ]* [^ ]*' || true
tail -n 1 "$out/host.log"
