# Sourced by board-scenarios-payload.sh: the scenarios that reset, damage or fill the cloned store
# (U7 resets, U10 bad_stores, U11 full_card). They only touch the clone and copies beside it
# ("$store"-*), which the payload's cleanup unmounts and removes, and run in the fuzz sandbox.
# shellcheck shell=bash disable=SC2024,SC2034,SC2154

reset_menu="$system down*9 enter up*20 down*3 enter up*5"
confirm="down*3 enter"
store_step() { settled_step store_state "$@"; }
main_pid() { systemctl show -p MainPID --value "$service"; }
patch_bpm() {
  sudo -n python3 -c 'import json, sys; print(round(json.load(open(sys.argv[1]))["runtimeConfig"]["transport"]["bpm"]))' \
    "$store/default.patch.json" 2>/dev/null
}

# Empty Patch, Factory Patch, then two System edits, Factory System with the restart declined, a
# service stop and start. The clone's USB and audio outputs start at factory values, so Factory
# System never asks root to apply a device change. A root loop checks every 20 ms that system.json
# never goes missing. On the Raspberry, Factory System turns Bluetooth off and with it the uinput
# keyboard, so the decline is never typed: the dialog stays open, and the MainPID must not change.
resets() {
  local start_sandbox=$sandbox pid
  prepare resets
  start || return
  store_state initial
  store_step empty "$reset_menu enter $confirm"
  store_step factory "$reset_menu down enter $confirm"
  sudo -n cat "$store/default.patch.json" > resets-factory-patch.json
  store_step system_edit "$display up*2 enter right*2 enter down enter right*2 enter"
  sudo -n sh -c "while [ ! -e $root/poll.stop ]; do [ -e $store/system.json ] || echo missing; sleep 0.02; done" > poll.log &
  pid=$(main_pid)
  store_step factory_system "$reset_menu down*2 enter $confirm wait:3 up*3 enter"
  echo "pid_before_reset=$pid pid_after_reset=$(main_pid)" >> stores.log
  sudo -n systemctl stop "$service"
  start
  touch "$root/poll.stop"
  wait
  store_state restarted
  echo "missing=$(grep -c missing poll.log)" >> stores.log
}

# Starts the service on a clone that should not load, waits for the restore screen, and records
# how often it was announced, whether systemd restarted the app and whether the files are intact.
broken_start() {
  local before since start_wait=0
  before=$(clone_hashes)
  since=$(date +%s)
  start || return
  for _ in $(seq 1 60); do
    sudo -n journalctl -u "$service" --since "@$since" --no-pager | grep -q "saved settings won't load" && break
    sleep 0.5
  done
  sleep 10
  echo "$1 announced=$(sudo -n journalctl -u "$service" --since "@$since" --no-pager | grep -c "saved settings won't load")" \
    "restarts=$(systemctl show -p NRestarts --value "$service") active=$(systemctl is-active "$service")" \
    "files_intact=$([ "$(clone_hashes)" = "$before" ] && echo yes || echo no)" >> stores.log
}
# A 0.8.15-style System file without midi.notesIn must load and save Notes In on; System files with
# an unknown key (top level and in runtimeConfig) and a truncated patch must each stop at the restore
# screen and keep the files. The screen's choices take the hardware encoders only, so the uinput
# keyboard cannot pick one.
bad_stores() {
  local start_sandbox=$sandbox setup
  prepare old_system
  start || return
  store_step old_system_saved "$system down enter up*5 down*2 enter up*5 enter $confirm"
  for setup in unknown_key unknown_runtime_key truncated_patch; do
    prepare "$setup"
    broken_start "$setup"
  done
}

# Moves the clone onto a 16 MB ext4 image, fills it to 48 kB free, then saves a preset and turns
# Grid Bright and BPM (Auto Save on). After the filler is removed, the retried autosave and a second
# Save As must land without a restart.
full_card() {
  local start_sandbox=$sandbox image="$store-card.img" free pid bpm copied
  local lib="$system down enter up*5 enter up*10"
  prepare full_card
  sudo -n mkdir "$store-copy" && sudo -n cp -a -- "$store/." "$store-copy/" || return
  sudo -n fallocate -l 16M "$image" && sudo -n mkfs.ext4 -q -m 0 "$image" && sudo -n mount -o loop "$image" "$store" || return
  sudo -n cp -a -- "$store-copy/." "$store/"
  copied=$(sudo -n diff -r "$store-copy" "$store" -x lost+found >/dev/null && echo yes || echo no)
  sudo -n chown "$user:$user" "$store"
  sudo -n chmod 0750 "$store"
  sudo -n install -d -o "$user" -g "$user" -m 0755 "$store/patches"
  start || return
  pid=$(main_pid)
  bpm=$(patch_bpm)
  free=$(sudo -n df -k --output=avail "$store" | tail -n 1 | tr -d ' ')
  sudo -n fallocate -l "$((free - 48))K" "$store/.filler"
  echo "filled copied=$copied free_kb=$(sudo -n df -k --output=avail "$store" | tail -n 1 | tr -d ' ')" >> stores.log
  store_step full_save_as "$lib enter up*5 down enter $confirm wait:3"
  mark full_edits_begin
  store_step full_edits "$display up*2 enter right*2 enter $link enter right*3 enter wait:5"
  sudo -n rm -f "$store/.filler"
  mark full_freed
  for _ in $(seq 1 60); do
    [ "$(patch_bpm)" = "$((bpm + 15))" ] && break
    sleep 0.5
  done
  store_state freed_autosave
  store_step freed_save_as "$lib enter up*5 down enter $confirm wait:3"
  echo "same_pid=$([ "$(main_pid)" = "$pid" ] && echo yes || echo no)" >> stores.log
}
