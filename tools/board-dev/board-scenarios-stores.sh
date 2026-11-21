# Sourced by board-scenarios-payload.sh: the scenarios that reset, damage or fill the cloned store
# (U7 resets, U10 bad_stores, U11 full_card). They only touch the clone and copies beside it
# ("$store"-*), which the payload's cleanup unmounts and removes.
# shellcheck shell=bash disable=SC2024,SC2034,SC2154

# One line of facts about the clone: layer behaviors, whether the patch is the live (factory) one,
# the System values the scenarios change, set-aside copies and leftover temporary files.
store_state() {
  sudo -n python3 - "$store" "$live" "$1" >> stores.log <<'PY'
import json, sys
from pathlib import Path
store, live, label = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3]
def music(path):
    """runtimeConfig without the delay times a note-mode delay derives from BPM."""
    config = json.loads(path.read_text())["runtimeConfig"]
    for bus in config.get("mixer", {}).get("buses", []):
        for slot in bus.values():
            if isinstance(slot, dict) and slot.get("params", {}).get("timeMode") == "note":
                slot["params"].pop("timeMs", None)
    return config
facts = [label]
try:
    patch = music(store / "default.patch.json")
    facts += [f"behaviors={','.join(sorted({l['build']['behaviorId'] for l in patch['layers']}))}",
              f"factory_patch={'yes' if patch == music(live / 'default.patch.json') else 'no'}",
              f"bpm={patch['transport']['bpm']}"]
except (OSError, ValueError, KeyError) as error:
    facts.append(f"patch_error={type(error).__name__}")
try:
    system = json.loads((store / "system.json").read_text())["runtimeConfig"]
    facts += [f"{key}={json.dumps(system[key], separators=(',', ':'))}" for key in
              ("gridBrightness", "buttonBrightness", "autoSaveDefault", "dimTimerSeconds", "screenSleepSeconds")]
    facts.append(f"notesIn={json.dumps(system['midi'].get('notesIn'))}")
except (OSError, ValueError, KeyError) as error:
    facts.append(f"system_error={type(error).__name__}")
names = sorted(p.name for p in store.rglob("*") if p.is_file() and p.name != ".filler")
facts.append(f"pre_reset={sum(n.startswith('system.json.pre-reset-') for n in names)}")
broken = []
for p in store.rglob("*.json"):
    try:
        json.loads(p.read_text()) if p.stat().st_size else broken.append(f"{p.name}:empty")
    except ValueError:
        broken.append(f"{p.name}:unparsable")
facts.append(f"broken={','.join(sorted(broken)) or '-'}")
facts.append(f"temporary={','.join(n for n in names if n.endswith('.tmp') or n.startswith('.')) or '-'}")
print(" ".join(facts))
PY
}
patch_bpm() {
  sudo -n python3 -c 'import json, sys; print(round(json.load(open(sys.argv[1]))["runtimeConfig"]["transport"]["bpm"]))' \
    "$store/default.patch.json" 2>/dev/null
}
store_hashes() { sudo -n sh -c "sha256sum $store/*.json $store/patches/*.json 2>/dev/null"; }
# Types the keys, then waits up to 15 s for the clone to change and hold still for 0.5 s.
store_step() {
  local before now settled=""
  before=$(store_hashes)
  keys "$1" "$2"
  for _ in $(seq 1 30); do
    now=$(store_hashes)
    [ "$now" != "$before" ] && [ "$now" = "$settled" ] && break
    settled=$now
    sleep 0.5
  done
  store_state "$1"
}
reset_menu="$system down*9 enter up*20 down*3 enter up*5"
confirm="down*3 enter"

# Empty Patch, Factory Patch, then two System edits, Factory System with the restart declined, a
# service stop and start. A root loop checks every 20 ms that system.json never goes missing.
resets() {
  configure autoSaveDefault=true dimTimerSeconds=0 screenSleepSeconds=0 perfViewOnBack=false
  start || return
  store_state initial
  store_step empty "$reset_menu enter $confirm"
  store_step factory "$reset_menu down enter $confirm"
  store_step system_edit "$display up*2 enter right*2 enter down enter right*2 enter"
  sudo -n sh -c "while [ ! -e $root/poll.stop ]; do [ -e $store/system.json ] || echo missing; sleep 0.02; done" > poll.log &
  store_step factory_system "$reset_menu down*2 enter $confirm wait:3 up*3 enter"
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
  before=$(store_hashes)
  since=$(date +%s)
  start || return
  for _ in $(seq 1 60); do
    sudo -n journalctl -u "$service" --since "@$since" --no-pager | grep -q "saved settings won't load" && break
    sleep 0.5
  done
  sleep 10
  echo "$1 announced=$(sudo -n journalctl -u "$service" --since "@$since" --no-pager | grep -c "saved settings won't load")" \
    "restarts=$(systemctl show -p NRestarts --value "$service") active=$(systemctl is-active "$service")" \
    "files_intact=$([ "$(store_hashes)" = "$before" ] && echo yes || echo no)" >> stores.log
}
# A 0.8.15-style System file without midi.notesIn must load and save Notes In on; a System file with
# an unknown key and a truncated patch must each stop at the restore screen and keep the files. The
# screen's choices take the hardware encoders only, so the uinput keyboard cannot pick one.
bad_stores() {
  prepare old_system
  start || return
  store_step old_system_saved "$system down enter up*5 down*2 enter up*5 enter $confirm"
  prepare unknown_key
  broken_start unknown_key
  prepare truncated_patch
  broken_start truncated_patch
}

# Moves the clone onto a 16 MB ext4 image, fills it to 48 kB free, then saves a preset, turns Grid
# Bright and BPM (Auto Save on). After the filler is removed, the retried autosave and a second
# Save As must land without a restart.
full_card() {
  local image="$store-card.img" free pid bpm
  prepare full_card
  sudo -n mkdir "$store-copy" && sudo -n cp -a -- "$store/." "$store-copy/" || return
  sudo -n fallocate -l 16M "$image" && sudo -n mkfs.ext4 -q -m 0 "$image" && sudo -n mount -o loop "$image" "$store" || return
  sudo -n cp -a -- "$store-copy/." "$store/"
  sudo -n chown "$user:$user" "$store"
  sudo -n chmod 0750 "$store"
  sudo -n install -d -o "$user" -g "$user" -m 0755 "$store/patches"
  start || return
  pid=$(systemctl show -p MainPID --value "$service")
  bpm=$(patch_bpm)
  free=$(sudo -n df -k --output=avail "$store" | tail -n 1 | tr -d ' ')
  sudo -n fallocate -l "$((free - 48))K" "$store/.filler"
  echo "filled free_kb=$(sudo -n df -k --output=avail "$store" | tail -n 1 | tr -d ' ')" >> stores.log
  local lib="$system down enter up*5 enter up*10"
  store_step full_save_as "$lib enter up*5 down enter $confirm wait:3"
  store_step full_edits "$display up*2 enter right*2 enter $link enter right*3 enter wait:5"
  sudo -n rm -f "$store/.filler"
  for _ in $(seq 1 60); do
    [ "$(patch_bpm)" = "$((bpm + 15))" ] && break
    sleep 0.5
  done
  store_state freed_autosave
  store_step freed_save_as "$lib enter up*5 down enter $confirm wait:3"
  echo "same_pid=$([ "$(systemctl show -p MainPID --value "$service")" = "$pid" ] && echo yes || echo no)" \
    "presets=$(sudo -n ls "$store/patches" | wc -l)" >> stores.log
}
