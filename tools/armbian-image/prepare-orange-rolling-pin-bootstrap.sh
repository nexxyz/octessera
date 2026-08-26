#!/usr/bin/env bash
set -euo pipefail

expected_framework_sha=3da49cffcb8ac58a919d86816fec4659c410ff1e

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 <armbian-build-directory> <candidate-capture-directory>" >&2
  exit 2
fi

build_input="$1"
capture_input="$2"
[[ -d "$build_input" && ! -L "$build_input" ]] || { echo 'Armbian build directory is missing or symlinked.' >&2; exit 1; }
build_dir="$(realpath -e -- "$build_input")"
[[ -f "$build_dir/compile.sh" && -x "$build_dir/compile.sh" && ! -L "$build_dir/compile.sh" ]] || { echo 'Armbian compile.sh is missing, not executable, or symlinked.' >&2; exit 1; }
[[ -d "$build_dir/config/sources" && ! -L "$build_dir/config/sources" ]] || { echo 'Armbian config/sources directory is missing or symlinked.' >&2; exit 1; }
for output_directory in "$build_dir/output" "$build_dir/output/info"; do
  if [[ -e "$output_directory" || -L "$output_directory" ]]; then
    [[ -d "$output_directory" && ! -L "$output_directory" ]] || { echo "Armbian output directory is unsafe: $output_directory" >&2; exit 1; }
  fi
done

capture_parent_input="$(dirname -- "$capture_input")"
capture_name="$(basename -- "$capture_input")"
[[ "$capture_name" =~ ^[A-Za-z0-9._-]+$ && "$capture_name" != . && "$capture_name" != .. ]] || { echo 'Candidate capture directory name is unsafe.' >&2; exit 1; }
[[ -d "$capture_parent_input" && ! -L "$capture_parent_input" ]] || { echo 'Candidate capture directory parent is missing or symlinked.' >&2; exit 1; }
capture_parent="$(realpath -e -- "$capture_parent_input")"
capture_dir="$capture_parent/$capture_name"
[[ ! -e "$capture_dir" && ! -L "$capture_dir" ]] || { echo 'Candidate capture directory already exists.' >&2; exit 1; }
case "$capture_dir/" in
  "$build_dir/"*) echo 'Candidate capture directory must be outside the Armbian build directory.' >&2; exit 1 ;;
esac

generated_source_lock="$build_dir/output/info/git_sources.json"
[[ ! -e "$generated_source_lock" ]] || { echo 'Refusing to use a reused artifact-config-dump source lock.' >&2; exit 1; }
actual_framework_head="$(git -C "$build_dir" rev-parse HEAD 2>/dev/null)" || { echo 'Unable to read Armbian framework HEAD.' >&2; exit 1; }
[[ "$actual_framework_head" == "$expected_framework_sha" ]] || { echo "Unexpected Armbian framework HEAD: $actual_framework_head" >&2; exit 1; }

(
  cd -- "$build_dir" || exit 1
  ./compile.sh artifact-config-dump-json WHAT=kernel BOARD=orangepizero2w RELEASE=trixie BRANCH=current REVISION=26.11.0-trunk.22 BUILD_DESKTOP=no BUILD_MINIMAL=yes KERNEL_CONFIGURE=no ENABLE_EXTENSIONS='octessera_midi octessera_audio octessera_sd2 octessera_image_sanitize' EXPERT=yes
)
[[ -f "$generated_source_lock" && ! -L "$generated_source_lock" ]] || { echo 'Candidate artifact config dump did not produce a regular source lock.' >&2; exit 1; }

python3 - "$generated_source_lock" <<'PY'
import json
import re
import sys

path = sys.argv[1]
def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result

with open(path, encoding="utf-8") as handle:
    sources = json.load(handle, object_pairs_hook=unique_object)
if not isinstance(sources, list) or len(sources) != 1:
    raise SystemExit("Candidate source lock must contain exactly one entry")
entry = sources[0]
if not isinstance(entry, dict) or set(entry) != {"source", "branch", "sha1"}:
    raise SystemExit("Candidate source lock entry must contain exactly source, branch, sha1")
if entry["source"] != "https://git.kernel.org/pub/scm/linux/kernel/git/stable/linux.git":
    raise SystemExit("Candidate source lock source is not the stable Linux Git source")
if entry["branch"] != "linux-6.18.y":
    raise SystemExit("Candidate source lock branch is not linux-6.18.y")
if not isinstance(entry["sha1"], str) or not re.fullmatch(r"[0-9a-f]{40}", entry["sha1"]):
    raise SystemExit("Candidate source lock SHA-1 must be lowercase full 40-hex")
PY

mkdir -- "$capture_dir"
captured_candidate_lock="$capture_dir/captured-candidate-source-lock.json"
cp -- "$generated_source_lock" "$captured_candidate_lock"
cmp -- "$generated_source_lock" "$captured_candidate_lock"
effective_source_lock="$build_dir/config/sources/git_sources.json"
[[ ! -L "$effective_source_lock" ]] || { echo 'Refusing to overwrite a symlinked effective source lock.' >&2; exit 1; }
cp -- "$generated_source_lock" "$effective_source_lock"
cmp -- "$generated_source_lock" "$effective_source_lock"

printf '%s\n' "Captured candidate source lock: $captured_candidate_lock"
