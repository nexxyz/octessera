#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"

bash "$root/tools/armbian-image/test-validation-runner.sh"
bash "$root/tools/armbian-image/test-validation-negative-fixtures.sh"

run_root_test() {
  local description="$1"
  shift
  if (( EUID == 0 )); then
    "$@"
  elif command -v sudo >/dev/null 2>&1 && sudo -n true >/dev/null 2>&1; then
    sudo -n env "PATH=$PATH" "CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}" "RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}" "$@"
  else
    echo "$description require root execution; run as root or configure passwordless sudo -n." >&2
    exit 1
  fi
}

bash "$root/tools/armbian-image/test-orange-oled-suspend.sh"
python3 "$root/tools/armbian-image/test-orange-oled-suspend.py"
python3 "$root/tools/armbian-image/test_orange_oled_logo.py"
python3 "$root/tools/armbian-image/test_orange_oled_off.py"
python3 "$root/tools/armbian-image/test_orange_oled_readiness.py"
python3 "$root/tools/armbian-image/test_orange_oled_handoff.py"
python3 "$root/tools/armbian-image/test_orange_oled_lifecycle.py"
python3 "$root/tools/armbian-image/test-orange-runtime-identity.py"
python3 "$root/tools/armbian-image/test-orange-construction.py"
run_root_test 'Orange updater image tests' python3 "$root/tools/armbian-image/test-orange-updater.py"
python3 -B "$root/tools/device-update/test_updater_layout.py"
python3 "$root/tools/armbian-image/test-orange-update-broker.py"
python3 "$root/tools/armbian-image/test-device-config.py"
python3 "$root/tools/armbian-image/test-orange-device-apply.py"
bash "$root/tools/armbian-image/test-orange-boot-splash-hook.sh"

bash "$root/tools/armbian-image/test-inspector.sh"
bash "$root/tools/armbian-image/test-image-mode.sh"
bash "$root/tools/armbian-image/test-orange-runtime-service.sh"
bash "$root/tools/armbian-image/test-orange-hdmi-rsyslog.sh"
bash "$root/tools/armbian-image/test-octessera-sd-card.sh"
bash "$root/tools/armbian-image/test-orange-alsa-sequencer.sh"
bash "$root/tools/armbian-image/test-orange-audio-extension.sh"
bash "$root/tools/armbian-image/test-orange-kernel-package.sh"
bash "$root/tools/armbian-image/test-orange-image-proof.sh"
bash "$root/tools/armbian-image/test-build-armbian-action.sh"
bash "$root/tools/armbian-image/test-release-workflow.sh"
run_root_test 'Musical asset provisioning tests' bash "$root/tools/armbian-image/test-musical-assets.sh"
bash "$root/tools/pi-image/test-wifi-foundation.sh"
bash "$root/tools/pi-image/test-rpi-boot-splash.sh"
bash "$root/tools/pi-image/test-rpi-boot-services.sh"
run_root_test 'Raspberry parent sudoers construction tests' bash "$root/tools/pi-image/test-rpi-sudoers-construction.sh"
run_root_test 'Raspberry initramfs proof tests' python3 "$root/tools/pi-image/test-rpi-initramfs-proof.py"
python3 "$root/tools/pi-image/test-boot-layer-contract.py"
run_root_test 'Raspberry sanitized-image boot-layout tests' bash "$root/tools/pi-image/test-sanitized-image-boot-layout.sh"
run_root_test 'Raspberry kernel image tests' bash "$root/tools/pi-image/test-rpi-kernel-image.sh"
bash "$root/tools/armbian-image/test-setup-layer.sh"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test_setup_sidecar.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-request.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-http.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-flow.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-state.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-readiness.py"
PYTHONDONTWRITEBYTECODE=1 python3 "$root/tools/armbian-image/test-setup-ui.py"

bash "$root/tools/armbian-image/resolve-armbian-extensions.sh" '' | grep -qxF 'octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'
bash "$root/tools/armbian-image/resolve-armbian-extensions.sh" preset-firstrun | grep -qxF 'preset-firstrun octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'
bash "$root/tools/armbian-image/resolve-armbian-extensions.sh" 'preset-firstrun octessera_midi' | grep -qxF 'preset-firstrun octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'
bash "$root/tools/armbian-image/resolve-armbian-extensions.sh" 'preset-firstrun,octessera_midi' | grep -qxF 'preset-firstrun,octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'
bash "$root/tools/armbian-image/resolve-armbian-extensions.sh" 'other-extension preset-firstrun' | grep -qxF 'other-extension preset-firstrun octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize'

cmp "$root/tools/device-update/octessera-update" "$root/userpatches/overlay/usr/local/sbin/octessera-update"
cmp "$root/tools/device-update/octessera-update-broker" "$root/userpatches/overlay/usr/local/sbin/octessera-update-broker"
cmp "$root/tools/device-update/octessera-update-guard" "$root/userpatches/overlay/usr/local/sbin/octessera-update-guard"
cmp "$root/tools/device-update/octessera-update-recovery" "$root/userpatches/overlay/usr/local/sbin/octessera-update-recovery"
octessera_reject_file_match 'Updater guard internals must not be present in sudoers.' -Eq 'octessera-update-(guard|recovery)' "$root/userpatches/overlay/etc/sudoers.d/octessera-update"
octessera_reject_file_match 'Updater recovery must run once per boot, not only when a transaction file exists.' -q '^ConditionPathExists=' "$root/userpatches/overlay/etc/systemd/system/octessera-update-recovery.service"
if [[ "${OCTESSERA_IMAGE_MODE:-diagnostic}" == diagnostic ]]; then
  diagnostic_runtime_paths="$(find "$root/userpatches/overlay" -type f -name octessera-pi -print)" || {
    echo 'Unable to inspect diagnostic Orange image runtime paths.' >&2
    exit 1
  }
  [[ -z "$diagnostic_runtime_paths" ]] || {
    echo 'Diagnostic Orange image overlay must not carry a runtime binary.' >&2
    exit 1
  }
fi
