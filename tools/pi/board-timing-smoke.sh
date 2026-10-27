#!/usr/bin/env bash
# Shared-path timing smoke on one board: plays the default patch with a candidate binary for a
# few minutes (OLED awake), then reports underruns, pulse cadence and runtime lateness.
# The board runs it detached, so there is no SSH traffic during the measurement.
#
#   tools/pi/board-timing-smoke.sh rpi pi@<PI_HOST> target/pi-cross/octessera-pi [seconds]
#   tools/pi/board-timing-smoke.sh opi octessera@<ORANGE_HOST> target/orange-pi-cross/octessera-pi
# Extra OCTESSERA_TIMING_*=value arguments after the seconds reach the candidate, e.g. the Aux
# study (OCTESSERA_TIMING_AUTOAUX=1 OCTESSERA_TIMING_KEEP_AWAKE=1) or the load ramp
# (OCTESSERA_TIMING_LOAD=start,step,max,seconds), which needs a -Profiling candidate binary;
# production binaries refuse it. OCTESSERA_TIMING_SOAK=1 keeps the store's own dim and screen-sleep
# settings, so long soaks let the OLED sleep. OCTESSERA_TIMING_OPTIMIZE_FOR=latency|capacity sets
# Perf. Mode in the cloned store for that run.
set -euo pipefail
board=${1:?board: rpi or opi} target=${2:?target: user@host} binary=${3:?candidate binary}
seconds=${4:-600}
shift $(($# < 4 ? $# : 4))
for setting in "$@"; do
  [[ "$setting" =~ ^OCTESSERA_TIMING_[A-Z_]+=[0-9A-Za-z,._-]+$ ]] || { echo "bad setting: $setting" >&2; exit 2; }
done
[[ "$board" == rpi || "$board" == opi ]] || { echo "board must be rpi or opi" >&2; exit 2; }
cd "$(git rev-parse --show-toplevel)"
remote() { powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/board-dev/with-board-ssh.ps1 -Board "$board" "$@"; }
local_path() { if command -v cygpath >/dev/null; then cygpath -w "$1"; else printf '%s' "$1"; fi; }
root="/tmp/octessera-smoke-$(date +%s)"
out="target/board-timing-smoke/$board-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"

profile=raspberry-pi-zero-2w
[[ "$board" == opi ]] && profile=orange-pi-zero-2w
{ printf "BOARD_PROFILE='%s'\n" "$profile"; cat tools/board-dev/board-check.sh; } > "$out/board-check.sh"
remote ssh-payload -Target "$target" "$(local_path "$out/board-check.sh")" || { echo "board check failed; not measuring" >&2; exit 1; }
remote ssh -Target "$target" "mkdir -m 755 $root"
remote scp -Target "$target" "$(local_path "$binary")" "$target:$root/octessera-pi" >/dev/null
remote scp -Target "$target" "$(local_path tools/pi/board-timing-smoke-payload.sh)" "$target:$root/payload.sh" >/dev/null
remote ssh -Target "$target" "chmod +x $root/octessera-pi; setsid nohup taskset -c 2,3 bash $root/payload.sh $board $seconds $root $* > $root/host.log 2>&1 < /dev/null &"
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
grep -E -- '-autoaux |timing-load step=|need the profiling build' "$journal" | tail -n 3 || true
tail -n 1 "$out/host.log"
