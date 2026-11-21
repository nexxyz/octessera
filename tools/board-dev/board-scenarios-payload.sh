#!/usr/bin/env bash
# Runs on the board, started by board-scenarios.sh. "boot" collects cold-boot evidence from the live
# service. "run" (started detached) drives the given scenarios with the virtual keyboard on the real
# service with a cloned store, then restores the service (and the USB gadget, if a scenario rebuilt it)
# and removes the clone and any recordings. The usb_ scenarios wait at gates for the host's part:
# the payload touches <gate>.ready and goes on once the host touches <gate>.go.
#
#   board-scenarios-payload.sh <rpi|opi> <root> boot
#   board-scenarios-payload.sh <rpi|opi> <root> run <soak-seconds> save_restore recording oled soak keymap presets
#     stop_save show_mode update_fail fuzz
#   board-scenarios-payload.sh <rpi|opi> <root> run 0 usb_midi|usb_midi_cc|usb_looper|usb_notes_in|usb_audio|usb_audio_fx
# Root reads privileged files; the redirects deliberately write as the unprivileged payload user.
# shellcheck disable=SC2024
set -u
board=$1 root=$2 mode=$3
cd "$root" || exit 1
service=octessera.service
ready=/run/octessera/candidate-ready.json
if [ "$board" = rpi ]; then
  user=pi live=/home/pi/presets media="/home/pi/recordings /home/pi/screen-recordings" bus=bt spi=0
  gadget_unit=octessera-usb-gadget.service
else
  user=octessera-runtime live=/var/lib/octessera/presets bus=usb spi=1
  gadget_unit=octessera-orange-usb-gadget.service
  media="/var/lib/octessera/recordings /var/lib/octessera/screen-recordings"
fi
hashes() { sudo -n find "$live" -maxdepth 1 -name '*.json' -exec sha256sum {} + | sort -k2; }
gadget_functions() { sudo -n sh -c 'ls /sys/kernel/config/usb_gadget/*/configs/*/ 2>/dev/null' | tr '\n' ' '; }

if [ "$mode" = boot ]; then
  pid=$(systemctl show -p MainPID --value "$service")
  {
    echo "service $(systemctl is-active "$service") pid $pid exe $(sudo -n sha256sum "/proc/$pid/exe" | cut -c1-16)"
    for pass in 1 2; do
      grep -H -E '^(state|hw_ptr)' /proc/asound/card*/pcm*p/sub*/status | sed "s/^/pass$pass /"
      sleep 1
    done
  } > boot-host.log 2>&1
  for _ in $(seq 1 30); do sudo -n grep -q "\"pid\": $pid," "$ready" 2>/dev/null && break; sleep 1; done
  sudo -n cat "$ready" > ready.json
  sudo -n journalctl -b -o short-monotonic --no-pager > boot-journal.log
  sudo -n journalctl -b -u "$service" -o json --no-pager > boot-service.json
  systemctl --failed --plain --no-legend > failed-units.log
  hashes > live-hashes.log
  exit 0
fi

soak_seconds=$4
shift 4
store=/var/lib/octessera/study-stores/scenarios-${root##*-}
dropin=/etc/systemd/system/octessera.service.d/95-octessera-scenarios.conf
trace=/sys/kernel/tracing/instances/octessera-scenarios
display="esc*8 up*10 down*10 enter up*20 down*7 enter down*20"
link="esc*8 up*10 down enter up*20"
system="esc*8 up*10 down*10 enter up*20"
mark() { echo "$(cut -d' ' -f1 /proc/uptime) $1" >> marks.log; }
keys() { sudo -n python3 "$root/board-keys.py" "$bus" "$root/marks.log" "$@"; }
snapshot() { sudo -n cat "$store/system.json" > "$1-system.json"; sudo -n cat "$store/default.patch.json" > "$1-patch.json"; }
configure() {
  sudo -n systemctl stop "$service"
  sudo -n python3 - "$store/system.json" "$@" <<'PY'
import json, sys
path = sys.argv[1]
system = json.load(open(path))
config = system["runtimeConfig"]
for pair in sys.argv[2:]:
    key, value = pair.split("=", 1)
    config[key] = json.loads(value)
# board-keys.py types on a uinput keyboard: off the terminal HDMI mode, and on the Raspberry over Bluetooth.
if config["hdmi"]["mode"] == "terminal":
    config["hdmi"]["mode"] = "live-grid"
config["bluetooth"]["enabled"] = True
json.dump(system, open(path, "w"), separators=(",", ":"))
PY
}
# With start_exec set, the service runs that binary instead of the installed one; start_sandbox
# adds these [Service] lines.
start() {
  sudo -n mkdir -p "${dropin%/*}"
  { printf '[Service]\nEnvironment=OCTESSERA_PI_STORE_DIR=%s %s\n' "$store" "$*"
    [ -z "$start_exec" ] || printf 'ExecStart=\nExecStart=%s\n' "$start_exec"
    [ -z "$start_sandbox" ] || printf '%s\n' "$start_sandbox"; } | sudo -n tee "$dropin" >/dev/null || return 1
  sudo -n systemctl daemon-reload
  # Never run a scenario against the live store.
  systemctl show -p Environment --value "$service" | grep -q "OCTESSERA_PI_STORE_DIR=$store" || { mark live_store_refused; return 1; }
  sudo -n systemctl restart "$service"
  for _ in $(seq 1 60); do
    sleep 1
    pid=$(systemctl show -p MainPID --value "$service")
    if sudo -n grep -q "\"pid\": $pid," "$ready" 2>/dev/null; then
      sleep 3
      mark "ready pid=$pid"
      return 0
    fi
  done
  mark ready_timeout
  return 1
}
# Rebuilds the USB gadget from the clone's System document; cleanup rebuilds it from the live one.
gadget_from_clone() {
  gadget_changed=1
  if [ "$board" = rpi ]; then
    sudo -n /usr/local/sbin/octessera-usb-gadget teardown
    sudo -n env OCTESSERA_USB_CONFIG="$store/system.json" /usr/local/sbin/octessera-usb-gadget setup
  else
    sudo -n /usr/local/sbin/octessera-orange-usb-gadget teardown
    sudo -n /usr/local/sbin/octessera-orange-usb-gadget setup --config "$store/system.json"
  fi
  echo "gadget from clone: $(gadget_functions)" >> host.log
}
# Waits with a builtin read: forking sleep on CPU 0 every second delayed USB MIDI clock-in by up to 2 ms.
gate() {
  mark "${1}_ready"
  touch "$root/$1.ready"
  [ -p "$root/gate.fifo" ] || { mkfifo "$root/gate.fifo" && exec 9<>"$root/gate.fifo"; }
  for _ in $(seq 1 900); do
    [ -e "$root/$1.go" ] && { mark "${1}_go"; return 0; }
    read -rt 1 -u 9
  done
  mark "${1}_timeout"
  return 1
}
prepare() {
  sudo -n systemctl stop "$service"
  sudo -n cp -a -- "$live/system.json" "$live/default.patch.json" "$store/"
  sudo -n python3 "$root/board-scenarios-store.py" "$store" "$1"
}
cleanup() {
  set +e
  # Nothing of the scenario may go on typing or switching releases once the drop-in is gone.
  sudo -n pkill -f "$root/board-keys.py"
  sudo -n pkill -f "$root/octessera-update switch"
  for _ in $(seq 1 20); do pgrep -f "$root/(board-keys.py|octessera-update)" >/dev/null || break; sleep 0.5; done
  if sudo -n test -d "$trace"; then
    sudo -n sh -c "echo 0 > $trace/events/spi/spi_transfer_start/enable"
    sudo -n rmdir "$trace"
  fi
  sudo -n journalctl -u "$service" --since "@$started" --no-pager -o short-monotonic > journal.log
  [ "$show_ran" = 1 ] && show_cleanup
  sudo -n rm -f "$dropin"
  sudo -n systemctl daemon-reload
  [ "$gadget_changed" = 1 ] && sudo -n systemctl restart "$gadget_unit"
  sudo -n systemctl restart "$service"
  sudo -n rm -rf -- "$store" "$store"-*
  # shellcheck disable=SC2086
  sudo -n find $media -type f -newer "$root/started" -print -delete >> host.log 2>&1
  for _ in $(seq 1 30); do [ "$(systemctl is-active "$service")" = active ] && break; sleep 1; done
  echo "restore service=$(systemctl is-active "$service") live_store_unchanged=$([ "$(hashes)" = "$original" ] && echo yes || echo NO) dropin_left=$(test -e "$dropin" && echo yes || echo no) gadget_restored=$([ "$(gadget_functions)" = "$gadget_original" ] && echo yes || echo NO)" >> host.log
  touch "$root/done"
}

save_restore() {
  configure autoSaveDefault=true dimTimerSeconds=0 screenSleepSeconds=0 gridBrightness=25 buttonBrightness=35 \
    'numericDisplayMode="bar"'
  snapshot save-before
  start || return
  keys save_edit "$display up*2 enter right*2 enter down enter right*2 enter down*20 up*11 enter right enter $link enter right*3 enter wait:8"
  snapshot save-edited
  start || return
  snapshot save-restarted
  keys save_runtime "$display up*2 enter right enter $link enter right enter wait:8"
  snapshot save-runtime
}
recording() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=0
  start OCTESSERA_TIMING_AUTOPLAY=1 || return
  keys rec "$system down*2 enter down*2 enter mark:start wait:20 up*20 down*3 enter mark:stop wait:8"
  # shellcheck disable=SC2086
  avi=$(sudo -n find $media -type f -name '*.avi' -newer "$root/started" | tail -n 1)
  [ -n "$avi" ] && sudo -n cat "$avi" > recording.avi
  echo "recording $avi" >> host.log
}
oled() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=10
  snapshot oled-before
  sudo -n mkdir "$trace"
  sudo -n sh -c "echo mono > $trace/trace_clock
    echo 'bus_num == $spi && chip_select == 0 && len < 4' > $trace/events/spi/spi_transfer_start/filter
    echo 1 > $trace/events/spi/spi_transfer_start/enable"
  mark oled_start
  start || return
  mark oled_ready
  sleep 30
  mark oled_asleep
  sleep 10
  keys oled "mark:wake shift wait:4 mark:woke $display up*3 enter mark:bright gap:1.5 right*4 gap:0.12 enter wait:3 mark:done"
  sudo -n cat "$trace/trace" > oled-trace.txt
}
soak() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=0
  start OCTESSERA_TIMING_AUTOPLAY=1 OCTESSERA_RUNTIME_TIMING_TRACE=1 OCTESSERA_PI_UI_PROFILE=1 || return
  pid=$(systemctl show -p MainPID --value "$service")
  minutes=$((soak_seconds / 60))
  # One root sampler for the whole soak: a sudo call per sample disturbs the audio path.
  sudo -n sh -c "for minute in \$(seq 0 $minutes); do
    echo \"\$minute \$(awk '/VmRSS/{print \$2}' /proc/$pid/status) \$(ls /proc/$pid/fd | wc -l)\"
    [ \$minute -lt $minutes ] && sleep 60
  done" > soak-proc.log &
  mark soak_start
  keys soak "esc*8 up*10 down enter down*12 up*12 enter esc down enter esc up esc down enter down*2 up*2 esc up*2 1*2 3*2 q*2 e*2 a*2 d*2" "$soak_seconds"
  wait
  mark soak_end
}
keymap() {
  prepare keymap
  start || return
  keys keymap "c v b n m , . / alt+c alt+f alt+r alt+4 alt+; wait:12"
  snapshot keymap
}
# One line per state: the saved patch's BPM and Note Length, the presets it matches, and a hash per
# preset file; presets-diff.log lists where the saved patch differs from preset A.
preset_state() {
  sudo -n python3 - "$store" "$1" >> presets.log <<'PY'
import hashlib, json, sys
from pathlib import Path
store = Path(sys.argv[1])
presets = sorted((store / "patches").glob("*.json"))
files = " ".join(f"{p.stem}={hashlib.sha256(p.read_bytes()).hexdigest()[:16]}" for p in presets)
def music(path):
    """A runtimeConfig without the delay times a note-mode delay derives from BPM: the runtime
    recomputes those on load (the factory patch stores 273 ms for a 1/8 at 120 BPM)."""
    config = json.loads(path.read_text())["runtimeConfig"]
    for bus in config.get("mixer", {}).get("buses", []):
        for slot in bus.values():
            if isinstance(slot, dict) and slot.get("params", {}).get("timeMode") == "note":
                slot["params"].pop("timeMs", None)
    return config
saved = music(store / "default.patch.json")
# The saved patch is a preset's when it holds every runtimeConfig value of that preset.
loaded = [p.stem for p in presets if all(saved.get(k) == v for k, v in music(p).items())]
def differ(a, b, path=""):
    if isinstance(a, dict) and isinstance(b, dict):
        return [d for k in sorted(set(a) | set(b)) for d in differ(a.get(k), b.get(k), f"{path}.{k}")]
    if isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        return [d for i, (x, y) in enumerate(zip(a, b)) for d in differ(x, y, f"{path}[{i}]")]
    return [] if a == b else [f"{path}={json.dumps(a)}->{json.dumps(b)}"]
if (store / "patches" / "A.json").exists():
    with open("presets-diff.log", "a") as out:
        a = music(store / "patches" / "A.json")
        out.write(f"{sys.argv[2]} {' '.join(differ(a, saved)[:20])}\n")
print(f"{sys.argv[2]} bpm={saved['transport']['bpm']} note={saved['sound']['noteLengthMs']} matches={','.join(loaded) or '-'} {files}")
PY
}
# Types one preset step and waits until the store changed and then held still for 0.5 s.
preset_step() {
  local before now settled
  before=$(sudo -n sh -c "sha256sum $store/patches/*.json $store/default.patch.json")
  keys "preset_$1" "$2"
  for _ in $(seq 1 30); do
    now=$(sudo -n sh -c "sha256sum $store/patches/*.json $store/default.patch.json")
    [ "$now" != "$before" ] && [ "$now" = "${settled:-}" ] && break
    settled=$now
    sleep 0.5
  done
  preset_state "$1"
}
# Text edits only append at the end of a name, so: Save As AC (the draft A comes from picking A in
# Rename), BPM +15, Load A over that unsaved change, Rename AC to ACD, Rename A onto AB with the
# overwrite confirmed, Delete ACD. Every confirm takes its last option.
presets() {
  prepare presets
  start || return
  local lib="$system down enter up*5 enter up*10" confirm="down*3 enter"
  preset_state initial
  preset_step save_as "$lib down*2 enter up*10 enter esc up*10 enter up*5 enter right*3 enter enter down enter $confirm"
  preset_step edit "$link enter right*3 enter"
  preset_step load "$lib down enter up*10 enter $confirm"
  preset_step rename "$lib down*2 enter up*10 down*2 enter down*20 up enter right*4 enter enter down enter $confirm"
  preset_step overwrite "$lib down*2 enter up*10 enter down*20 up enter right*2 enter enter down enter $confirm"
  preset_step delete "$lib down*3 enter up*10 down enter $confirm"
}
# Waits up to $2 s for the mark $1; on a timeout it marks <mark>_timeout and fails.
await_mark() {
  local tries=$(($2 * 50))
  until grep -q " $1\$" marks.log 2>/dev/null; do
    tries=$((tries - 1))
    [ "$tries" -gt 0 ] || { mark "${1}_timeout"; return 1; }
    sleep 0.02
  done
}
# Stops the service once the mark $1 is in the log (waiting up to $2 s), and logs when the stop was
# issued (uptime, the marks' clock) and how long it took.
stop_after() {
  local since
  await_mark "$1" "$2" || return 1
  since=$(date +%s%N)
  echo "$1 issued=$(cut -d' ' -f1 /proc/uptime)" >> stop.log
  sudo -n systemctl stop "$service"
  echo "$1 stop_ms=$((($(date +%s%N) - since) / 1000000))" >> stop.log
}
# Grid Bright +10 and BPM +15, then a stop right after the last key; then 5 s of Audio recording and
# a stop. Only a .wav in the Audio recordings folder, newer than this take's marker, counts.
stop_save() {
  local recordings=${media%% *} rec
  configure autoSaveDefault=true dimTimerSeconds=0 screenSleepSeconds=0
  snapshot stop-before
  start || return
  keys stop_edit "$display up*2 enter right*2 enter $link enter right*3 gap:0.05 enter mark:edited" &
  stop_after stop_edit_edited 30 || { wait; return; }
  wait
  snapshot stop-saved
  start || return
  touch "$root/stop-rec-started"
  keys stop_rec "$system down*2 enter down enter mark:recording" &
  await_mark stop_rec_recording 60 || { wait; return; }
  sleep 5
  mark stop_rec_stopping
  stop_after stop_rec_stopping 5
  wait
  sudo -n find "$recordings" -maxdepth 1 -type f -name '*.wav' -newer "$root/stop-rec-started" -printf '%p %s\n' >> stop.log
  echo "partial=$(sudo -n find "$recordings" -maxdepth 1 -type f -name '*.partial.wav' -newer "$root/stop-rec-started" | wc -l)" >> stop.log
  rec=$(sudo -n find "$recordings" -maxdepth 1 -type f -name '*.wav' ! -name '*.partial.wav' -newer "$root/stop-rec-started" | tail -n 1)
  [ -n "$rec" ] && sudo -n cat "$rec" > stop-recording.media
}
rss() { sudo -n awk '/VmRSS/{print $2}' "/proc/$(systemctl show -p MainPID --value "$service")/status"; }
# Five minutes of seeded random input on the store's own patch. The service cannot reach root's
# power, device-apply, update, setup-portal, Orange storage-control (SD2 transfer) or Live Show
# Mode paths while it runs. A restart afterwards shows the clone still loads.
fuzz() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=0
  local seed=${OCTESSERA_SCENARIO_FUZZ_SEED:-$(date +%s)} start_sandbox restarted=no
  start_sandbox="NoNewPrivileges=yes
InaccessiblePaths=-/run/octessera-device-apply -/run/octessera-update -/run/octessera-setup-request/inbox
InaccessiblePaths=-/run/octessera-orange-storage-control -/var/lib/octessera/show-hold
InaccessiblePaths=-/usr/bin/systemctl -/usr/sbin/reboot -/usr/sbin/poweroff"
  start OCTESSERA_TIMING_AUTOPLAY=1 || return
  echo "start seed=$seed pid=$(systemctl show -p MainPID --value "$service") rss_kb=$(rss)" >> fuzz.log
  keys fuzz "random:$seed" 300
  echo "end pid=$(systemctl show -p MainPID --value "$service") rss_kb=$(rss) active=$(systemctl is-active "$service")" >> fuzz.log
  mark fuzz_typed
  start OCTESSERA_TIMING_AUTOPLAY=1 && restarted=yes
  echo "restart ready=$restarted" >> fuzz.log
}
usb_midi() {
  local setup
  for setup in usb_midi usb_midi_swing; do
    prepare "$setup"
    [ "$setup" = usb_midi ] && gadget_from_clone
    start OCTESSERA_RUNTIME_TIMING_TRACE=1 OCTESSERA_PI_UI_PROFILE=1 || return
    sleep 5
    gate "$setup" || return
  done
}
# One host_usb_midi.py phase on a clone set up for it.
usb_midi_phase() {
  prepare "$1"
  gadget_from_clone
  start OCTESSERA_RUNTIME_TIMING_TRACE=1 || return
  sleep 5
  gate "$1"
}
usb_midi_cc() { usb_midi_phase usb_midi_cc; }
usb_notes_in() { usb_midi_phase usb_notes_in; }
usb_looper() {
  usb_midi_phase usb_looper || return
  # Auto Save writes the recorded steps: wait for the four presses and releases.
  for _ in $(seq 1 30); do
    [ "$(sudo -n grep -Eo '"kind": ?"(press|release)"' "$store/default.patch.json" | wc -l)" -ge 8 ] && break
    sleep 1
  done
  snapshot looper
}
# Holds the tape stop on Play FX tile (3, 3), key 7, while the host captures USB Audio.
usb_audio_fx() {
  prepare usb_audio_fx
  gadget_from_clone
  start OCTESSERA_TIMING_AUTOPLAY=1 || return
  gate usb_audio_fx || return
  keys fx "esc*8 wait:5 shift+ctrl+/ wait:2 mark:hold hold:7:6 mark:release wait:20"
}
usb_audio() {
  prepare usb_audio
  gadget_from_clone
  start OCTESSERA_TIMING_AUTOPLAY=1 OCTESSERA_RUNTIME_TIMING_TRACE=1 OCTESSERA_PI_UI_PROFILE=1 || return
  gate usb_audio || return
  prepare usb_audio_clip
  start OCTESSERA_TIMING_AUTOPLAY=1 || return
  gate usb_audio_clip || return
  sleep 5
  keys clip "$system down*2 enter down*2 enter mark:start wait:12 $system down*4 enter up*20 down*2 enter gap:0.02 left*16 gap:0.12 mark:lowered enter wait:8 $system down*2 enter up*20 down*3 enter mark:stop wait:8"
  # shellcheck disable=SC2086
  avi=$(sudo -n find $media -type f -name '*.avi' -newer "$root/started" | tail -n 1)
  [ -n "$avi" ] && sudo -n cat "$avi" > clip.avi
  echo "recording $avi" >> host.log
}

# shellcheck disable=SC1091
. "$root/board-scenarios-system.sh"
started=$(date +%s)
gadget_changed=0
show_ran=0
start_exec=""
start_sandbox=""
gadget_original=$(gadget_functions)
touch started
original=$(hashes)
trap cleanup EXIT
sudo -n install -d -o root -g root -m 0755 /var/lib/octessera/study-stores
sudo -n install -d -o "$user" -g "$user" -m 0750 "$store"
sudo -n cp -a -- "$live/." "$store/"
sudo -n modprobe uinput
for scenario in "$@"; do
  mark "${scenario}_begin"
  "$scenario"
  mark "${scenario}_end"
done
