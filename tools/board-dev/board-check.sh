#!/usr/bin/env bash
# Runs on the board, piped over SSH; installs and writes nothing. The caller prepends BOARD_PROFILE.
# Prints one PASS/FAIL line per check and exits non-zero if any check fails.
set -uo pipefail

service=octessera.service
case "${BOARD_PROFILE:-}" in
  raspberry-pi-zero-2w)
    model='Raspberry Pi Zero 2 W' contract=/etc/octessera/board-profile.env user=pi
    store=/home/pi/presets samples=/home/pi/samples i2c=/dev/i2c-1 spi=/dev/spidev0.0
    usb_unit=octessera-usb-gadget.service udc='' ;;
  orange-pi-zero-2w)
    model='OrangePi Zero 2W' contract=/etc/octessera/build-metadata.env user=octessera-runtime
    store=/var/lib/octessera/presets samples=/var/lib/octessera/samples i2c=/dev/i2c-2 spi=/dev/spidev1.0
    usb_unit=octessera-orange-usb-gadget.service udc=musb-hdrc.4.auto ;;
  *) echo "FAIL board-profile: unknown BOARD_PROFILE '${BOARD_PROFILE:-}'"; exit 1 ;;
esac
failed=0
check() { if [[ $2 == ok ]]; then echo "PASS $1"; else echo "FAIL $1: $2"; failed=1; fi; }
read_root() { sudo -n cat -- "$1" 2>/dev/null; }
json_value() { grep -o "\"$2\": *\"\?[^\",}]*" <<<"$1" | head -n 1 | sed 's/^[^:]*: *"\?//'; }
property() { systemctl show "$service" --property="$1" --value 2>/dev/null; }

if [[ "$(uname -s)" == Linux ]]; then check platform ok; else check platform 'not a Linux host'; fi

reason=ok
if [[ "$(tr -d '\0' < /proc/device-tree/model 2>/dev/null)" != *"$model"* ]]; then reason="model is not $model"
elif ! grep -qx "OCTESSERA_BOARD_PROFILE_ID=$BOARD_PROFILE" "$contract" 2>/dev/null; then reason="$contract does not select $BOARD_PROFILE"
elif [[ ! -e $i2c || ! -e $spi ]]; then reason="$i2c or $spi is missing"; fi
check identity "$reason"

reason=ok
if [[ "$(systemctl is-enabled "$service" 2>/dev/null)" != enabled ]]; then reason="$service is not enabled"
elif [[ "$(systemctl is-active "$service" 2>/dev/null)" != active ]]; then reason="$service is not active"
elif [[ "$(property User)" != "$user" ]]; then reason="$service does not run as $user"; fi
check service "$reason"

# A just-restarted runtime needs a few seconds to bring audio and hardware up; the updater allows 60.
for _ in $(seq 1 60); do
  marker=$(read_root /run/octessera/candidate-ready.json)
  reason=ok
  if [[ -z $marker ]]; then reason='/run/octessera/candidate-ready.json is missing'
  elif [[ "$(json_value "$marker" schema_version)" != 1 || "$(json_value "$marker" board_profile)" != "$BOARD_PROFILE" ]]; then
    reason="readiness marker is not a schema 1 $BOARD_PROFILE marker"
  elif [[ "$(json_value "$marker" pid)" != "$(property MainPID)" || "$(json_value "$marker" systemd_invocation_id)" != "$(property InvocationID)" ]]; then
    reason='readiness marker is stale for the running service'; fi
  [[ $reason == ok ]] && break
  sleep 1
done
check readiness "$reason"
readiness=$reason

reason=ok
if [[ ! -d $store || ! -d $samples || ! -f $store/system.json || ! -f $store/default.patch.json ]]; then
  reason="$store, $samples, system.json or default.patch.json is missing"
elif [[ -e $store/backups && ! -d $store/backups ]]; then reason="$store/backups is not a directory"; fi
check storage "$reason"

setup=/run/octessera-setup-status
reason=ok
if [[ -e $setup && ! -d $setup ]]; then reason="$setup is not a directory"
elif [[ -e $setup/current.json ]]; then
  receipt=$(read_root "$setup/current.json")
  if [[ "$(json_value "$receipt" schema)" != 1 || "$receipt" != *'"status"'* ]]; then reason='setup status receipt is not schema 1'
  elif [[ "$(json_value "$receipt" phase)" == unsupported ]]; then reason='setup status phase is unsupported'; fi
fi
check setup-status "$reason"

reason=ok
if [[ $BOARD_PROFILE == raspberry-pi-zero-2w ]]; then
  handoff=$(read_root /run/octessera-boot/status.json)
  phase=$(json_value "$handoff" phase)
  if [[ "$(json_value "$handoff" schema)" != 1 || ! ( $phase == native_owned || $phase == first_menu_rendered ) ]]; then
    reason="OLED handoff is not in native ownership (phase=${phase:-none})"
  elif ! sudo -n test -e /run/octessera-boot/oled.lock; then reason='OLED handoff lock is missing'; fi
elif [[ $readiness != ok ]]; then reason='Orange handoff is covered by readiness, which failed'; fi
check oled-handoff "$reason"

reason=ok
if [[ $BOARD_PROFILE == raspberry-pi-zero-2w ]]; then
  aplay -l 2>/dev/null | grep -Eqi 'hifiberry|pcm5102a|snd_rpi_hifiberry' || reason='no HiFiBerry/PCM5102A card in aplay -l'
else
  card=$(grep -E '^ *[0-9]+ \[octesseradac *\]: octessera-dac - octessera-dac$' /proc/asound/cards 2>/dev/null)
  index=$(sed -E 's/^ *([0-9]+).*/\1/' <<<"$card")
  if [[ $(grep -c . <<<"$card") -ne 1 || -z $card ]]; then reason='octesseradac card is not listed exactly once'
  elif [[ $(grep -cE "^$(printf '%02d' "$index")-00: .*: playback 1( |$)" /proc/asound/pcm) -ne 1 ]]; then
    reason='octesseradac playback PCM is missing'; fi
fi
check audio "$reason"

reason=ok
if [[ ! -d /sys/class/udc || ! -d /sys/kernel/config/usb_gadget ]]; then reason='USB UDC or configfs is unavailable'
elif [[ -n $udc && ! -e /sys/class/udc/$udc ]]; then reason="UDC $udc is missing"
elif ! compgen -G '/sys/class/udc/*' >/dev/null; then reason='no USB device controller'
elif [[ "$(systemctl is-active "$usb_unit" 2>/dev/null)" != active ]]; then reason="$usb_unit is not active"; fi
check usb "$reason"

exit "$failed"
