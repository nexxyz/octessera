# Sourced by board-scenarios-payload.sh: the scenarios that drive root-owned board state, Live Show
# Mode and the updater's release switch. Each keeps its changes in a copy beside the cloned store
# ("$store"-*), which the payload's cleanup removes; show_mode also ends any show it left.
# shellcheck shell=bash disable=SC2024,SC2034,SC2154

show_dir=/var/lib/octessera/show-hold
show_apply=/usr/local/sbin/octessera-show-hold-apply
wlan_blocked() { sudo -n rfkill list wlan | grep -q 'Soft blocked: yes' && echo yes || echo no; }
show_state() {
  echo "$1 length=$(sudo -n cat "$show_dir/length" 2>/dev/null | tr -d '\n' || true)" \
    "elapsed=$(sudo -n sed -n 2p "$show_dir/elapsed" 2>/dev/null || true) wlan_blocked=$(wlan_blocked)" \
    "release_timers=$(systemctl list-timers --all --no-legend 'octessera-show-hold-release-*' | grep -c 'octessera-show-hold-release-' || true)" \
    "health_timer=$(systemctl is-active octessera-network-health.timer 2>/dev/null || true)" >> show.log
}
# Waits until the radio is blocked or unblocked, up to $2 seconds, and logs how long it took.
await_wlan() {
  local since=$SECONDS
  while [ "$(wlan_blocked)" != "$1" ] && [ $((SECONDS - since)) -lt "$2" ]; do sleep 0.2; done
  echo "$3 wlan_blocked=$(wlan_blocked) after=$((SECONDS - since))s" >> show.log
}
# Ends a show the scenario left running, so a failed run never keeps the board off Wi-Fi.
show_cleanup() {
  grep -q " $show_apply " /proc/self/mountinfo && sudo -n umount "$show_apply"
  if sudo -n test -e "$show_dir/length"; then
    sudo -n rm -f "$show_dir/length" "$show_dir/elapsed"
    sudo -n "$show_apply"
  fi
}

# Starts a show from the menu, shortens it to 70 s and counts it down in 20 s ticks (the installed
# apply script, bind-mounted with only its step changed), restarts the service at 45 s, checks the
# show ended by itself at 100 s, then starts a second show and ends it from the menu.
show_mode() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=0
  sed 's/^step=300$/step=20/' "$show_apply" | sudo -n tee "$store-show-hold-apply" >/dev/null
  sudo -n chmod 755 "$store-show-hold-apply"
  grep -q '^step=20$' "$store-show-hold-apply" || { echo "step override missing" >> show.log; return; }
  show_ran=1
  sudo -n mount --bind "$store-show-hold-apply" "$show_apply" || return
  start || return
  local live_show="$system down*8 enter up*5"
  keys show_start "$live_show enter down enter"
  await_wlan yes 10 started
  # The runtime writes the length as its own user; a new length is a new show.
  printf '70\n' | sudo -n -u "$user" tee "$show_dir/length" >/dev/null
  local zero=$SECONDS
  mark show_shortened
  sleep 45
  show_state at45
  sudo -n systemctl restart "$service"
  mark show_restarted
  local left=$((100 - (SECONDS - zero)))
  [ "$left" -gt 0 ] && sleep "$left"
  show_state at100
  sudo -n journalctl --since "@$started" --no-pager -o short-monotonic -u 'octessera-show-hold*' > show-journal.log
  keys show_second "$live_show enter down enter"
  await_wlan yes 10 second_started
  sleep 10
  keys show_end "$live_show down enter"
  await_wlan no 5 ended
  show_state after_end
  show_cleanup
}

update_root=""
# Runs this tree's updater's release switch, as octessera-update-switch@ does, against scratch
# releases beside the clone: base and good are the binary under test, dies exits 1, hangs ignores
# SIGTERM and never reports ready.
update_step() {
  local name=$1 version=$2 before after message rc since
  before=$(sudo -n sha256sum "$store/system.json" "$store/default.patch.json" | cut -c1-64 | tr '\n' ' ')
  since=$SECONDS
  message=$(sudo -n env OCTESSERA_UPDATE_ROOT="$update_root" OCTESSERA_UPDATE_READY_SECONDS=45 \
    python3 "$root/octessera-update" switch "$version" 2>&1)
  rc=$?
  message=${message//$'\n'/ | }
  local took=$((SECONDS - since))
  since=$SECONDS
  while [ $((SECONDS - since)) -lt 120 ]; do
    pid=$(systemctl show -p MainPID --value "$service")
    [ "$pid" != 0 ] && sudo -n grep -q "\"pid\": $pid," "$ready" 2>/dev/null && break
    sleep 1
  done
  after=$(sudo -n sha256sum "$store/system.json" "$store/default.patch.json" | cut -c1-64 | tr '\n' ' ')
  echo "$name rc=$rc took=${took}s ready_after=$((SECONDS - since))s active=$(systemctl is-active "$service")" \
    "current=$(basename "$(readlink "$update_root/current")") store_unchanged=$([ "$before" = "$after" ] && echo yes || echo no)" \
    "message=$message" >> update.log
}
# A scratch release: a link to the binary under test, or a script with the given body.
fake_release() {
  sudo -n install -d -m 0755 "$update_root/releases/$1"
  if [ "${2:0:1}" = / ]; then
    sudo -n ln -s "$2" "$update_root/releases/$1/octessera-pi"
  else
    printf '#!/bin/sh\n%s\n' "$2" | sudo -n tee "$update_root/releases/$1/octessera-pi" >/dev/null
    sudo -n chmod 755 "$update_root/releases/$1/octessera-pi"
  fi
}
update_fail() {
  configure autoSaveDefault=false dimTimerSeconds=0 screenSleepSeconds=0
  local binary
  binary=$(readlink -f "$(systemctl show -p ExecStart --value "$service" | sed -n 's/.*path=\([^ ;]*\).*/\1/p')")
  echo "binary $binary" >> update.log
  update_root="$store-update"
  fake_release 0.9.90 "$binary"
  fake_release 0.9.91 "$binary"
  sudo -n ln -s "$update_root/releases/0.9.90" "$update_root/current"
  start_exec="$update_root/current/octessera-pi"
  start || return
  update_step good 0.9.91
  update_step back 0.9.90
  # A good switch prunes every release but the two it touched, so the failing ones come after.
  fake_release 0.9.92 'exit 1'
  update_step dies 0.9.92
  fake_release 0.9.93 'trap "" TERM; while :; do sleep 1; done'
  update_step hangs 0.9.93
  start_exec=""
}
