#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$root/tools/armbian-image/validation-assertions.sh"
extension="$root/userpatches/extensions/octessera_midi.sh"
module_file="$root/userpatches/overlay/etc/modules-load.d/octessera-orange-midi.conf"
customize="$root/userpatches/customize-image.sh"

[[ -f "$extension" ]] || { echo "Missing ALSA sequencer kernel extension." >&2; exit 1; }
[[ -f "$module_file" ]] || { echo "Missing Orange ALSA module-load file." >&2; exit 1; }
bash -n "$extension"

# shellcheck source=userpatches/extensions/octessera_midi.sh
source "$extension"
opts_n=()
opts_y=()
opts_m=()
declare -A opts_val=()
custom_kernel_config__octessera_midi
expected_options=(SND_SEQUENCER SND_RAWMIDI SND_USB_AUDIO)
[[ "${#opts_n[@]}" == 0 ]] || { echo "ALSA extension must not use ineffective opts_n for RT_GROUP_SCHED." >&2; exit 1; }
[[ "${#opts_y[@]}" == 0 ]] || { echo "ALSA extension must not enable RT_GROUP_SCHED." >&2; exit 1; }
[[ "${opts_val[RT_GROUP_SCHED]:-}" == n ]] || { echo "ALSA extension must force RT_GROUP_SCHED=n through opts_val." >&2; exit 1; }
[[ "${#opts_m[@]}" == "${#expected_options[@]}" ]] || { echo "ALSA extension requested an unexpected number of options." >&2; exit 1; }
for index in "${!expected_options[@]}"; do
    [[ "${opts_m[$index]}" == "${expected_options[$index]}" ]] || {
        echo "ALSA extension requested an unexpected kernel option: ${opts_m[$index]}" >&2
        exit 1
    }
done

octessera_reject_file_match "ALSA kernel fix must not change global RT throttling, cgroup mode, or capabilities." -Eiq 'sched_rt_(period|runtime)|cgroup|CAP_SYS_NICE|CapabilityBoundingSet|AmbientCapabilities' "$extension" "$module_file" "$customize"

octessera_reject_file_match "ALSA extension contains obsolete, OSS, or generic discovery behavior." -Eq 'CONFIG_SND_SEQ([_=[:space:]]|$)|SND_.*OSS|modprobe|lsmod|udevadm|/sys/class|/dev/snd' "$extension"

printf '%s\n' snd_seq snd_seq_midi | cmp -s - "$module_file" || {
    echo "Orange ALSA module-load content is not the minimal exact list." >&2
    exit 1
}
octessera_reject_file_match "Orange ALSA module-load file contains generic discovery or OSS behavior." -Eq 'find|lsmod|modprobe|udevadm|/sys/class|/dev/snd|snd_seq_oss|snd-seq-oss' "$module_file"

printf 'Orange ALSA sequencer image tests passed\n'
