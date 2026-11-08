#!/usr/bin/env bash
# Runs on the board, started by board-scenarios.sh. "boot" collects cold-boot evidence from the live
# service. "run" (started detached) drives the given scenarios with the virtual keyboard on the real
# service with a cloned store, then restores the service (and the USB gadget, if a scenario rebuilt it)
# and removes the clone and any recordings. The usb_ scenarios wait at gates for the host's part:
# the payload touches <gate>.ready and goes on once the host touches <gate>.go.
#
#   board-scenarios-payload.sh <rpi|opi> <root> boot
#   board-scenarios-payload.sh <rpi|opi> <root> run <soak-seconds> save_restore recording oled soak keymap
#   board-scenarios-payload.sh <rpi|opi> <root> run 0 usb_midi|usb_midi_cc|usb_audio|usb_audio_fx
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
start() {
  sudo -n mkdir -p "${dropin%/*}"
  printf '[Service]\nEnvironment=OCTESSERA_PI_STORE_DIR=%s %s\n' "$store" "$*" | sudo -n tee "$dropin" >/dev/null || return 1
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
  if sudo -n test -d "$trace"; then
    sudo -n sh -c "echo 0 > $trace/events/spi/spi_transfer_start/enable"
    sudo -n rmdir "$trace"
  fi
  sudo -n journalctl -u "$service" --since "@$started" --no-pager -o short-monotonic > journal.log
  sudo -n rm -f "$dropin"
  sudo -n systemctl daemon-reload
  [ "$gadget_changed" = 1 ] && sudo -n systemctl restart "$gadget_unit"
  sudo -n systemctl restart "$service"
  sudo -n rm -rf -- "$store"
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
  keys save_edit "$display up*2 enter right*2 enter down enter right*2 enter down*20 up*9 enter right enter $link enter right*3 enter wait:8"
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
usb_midi_cc() {
  prepare usb_midi_cc
  gadget_from_clone
  start OCTESSERA_RUNTIME_TIMING_TRACE=1 || return
  sleep 5
  gate usb_midi_cc || return
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

started=$(date +%s)
gadget_changed=0
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
