#!/usr/bin/env bash
# shellcheck disable=SC2154
module_dir="$(dirname "${BASH_SOURCE[0]}")"
# shellcheck source=tools/armbian-image/validation-assertions.sh
source "$module_dir/validation-assertions.sh"

octessera_require_device_tree_contract() {
  local profile_metadata="$1"
  local spi_source_path=usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts
  local spi_dtbo_path=boot/overlay-user/octessera-h618-spi1-cs0.dtbo
  local input_source_path=usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts
  local input_dtbo_path=boot/overlay-user/octessera-h618-input-routing.dtbo
  local armbian_env_path=boot/armbianEnv.txt path source_hash dtbo_hash input_source_hash input_dtbo_hash
  local armbian_env_content spi_source_content input_source_content
  for path in "$spi_source_path" "$spi_dtbo_path" "$input_source_path" "$input_dtbo_path" "$armbian_env_path"; do
    stat_path "$path" || { echo "Missing Orange Pi SPI image path: $path." >&2; exit 1; }
  done
  for path in "$spi_source_path" "$spi_dtbo_path" "$input_source_path" "$input_dtbo_path" "$armbian_env_path"; do require_root_mode "$path" 644; done
  source_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTS_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
  dtbo_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_SPI1_CS0_DTBO_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
  input_source_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_INPUT_ROUTING_DTS_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
  input_dtbo_hash="$(printf '%s\n' "$profile_metadata" | sed -n 's/^OCTESSERA_INPUT_ROUTING_DTBO_SHA256=\([a-fA-F0-9]\{64\}\)$/\1/p')"
  [[ -n "$source_hash" && -n "$dtbo_hash" && -n "$input_source_hash" && -n "$input_dtbo_hash" ]] || { echo 'Armbian image is missing device-tree hashes.' >&2; exit 1; }
  [[ "$(hash_path "$spi_source_path")" == "$source_hash" ]] || { echo 'SPI overlay source hash mismatch.' >&2; exit 1; }
  [[ "$(hash_path "$spi_dtbo_path")" == "$dtbo_hash" ]] || { echo 'SPI overlay DTBO hash mismatch.' >&2; exit 1; }
  [[ "$(hash_path "$input_source_path")" == "$input_source_hash" ]] || { echo 'Input-routing overlay source hash mismatch.' >&2; exit 1; }
  [[ "$(hash_path "$input_dtbo_path")" == "$input_dtbo_hash" ]] || { echo 'Input-routing overlay DTBO hash mismatch.' >&2; exit 1; }
  armbian_env_content="$(read_file "$armbian_env_path")"
  validate_env_tokens "$armbian_env_content" overlays i2c1-pi || { echo 'Armbian image must claim overlays=i2c1-pi exactly once.' >&2; exit 1; }
  validate_env_tokens "$armbian_env_content" user_overlays octessera-h618-spi1-cs0 || { echo 'Armbian image must claim the SPI user overlay exactly once.' >&2; exit 1; }
  validate_env_tokens "$armbian_env_content" user_overlays octessera-h618-input-routing || { echo 'Armbian image must claim the input-routing user overlay exactly once.' >&2; exit 1; }
  console_matches="$(printf '%s\n' "$armbian_env_content" | awk '!/^[[:space:]]*#/ && /(^|[[:space:]])console=ttyS0(,|$)/')" || {
    echo 'Unable to inspect Armbian console arguments.' >&2
    exit 1
  }
  [[ -z "$console_matches" ]] || { echo 'Armbian image must not retain console=ttyS0 in armbianEnv.txt.' >&2; exit 1; }
  spi_source_content="$(read_file "$spi_source_path")"
  spi_forbidden_content="$spi_source_content
$armbian_env_content"
  octessera_reject_text_match 'SPI image integration must not contain the stock spidev fallback.' "$spi_forbidden_content" -q 'spidev1_0'
  input_source_content="$(read_file "$input_source_path")"
  printf '%s\n' "$input_source_content" | grep -q 'status = "disabled"'
  printf '%s\n' "$input_source_content" | grep -q 'pins = "PH0", "PH1"'
  printf '%s\n' "$input_source_content" | grep -q 'stdout-path = ""'
}

validate_env_tokens() {
  local content="$1" key="$2" required_token="$3"
  printf '%s\n' "$content" | awk -v key="$key" -v required_token="$required_token" '
    function invalid(message) { print "Invalid " key " assignment: " message > "/dev/stderr"; failed = 1 }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) { if (line ~ ("(^|[^_[:alnum:]])" key "[[:space:]]*=")) invalid("commented assignment"); next }
      if (line ~ ("^" key "=")) {
        if (assignments++) invalid("duplicate assignment")
        value = substr(line, length(key) + 2)
        if (value ~ /#/) invalid("comments are not allowed")
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        count = value == "" ? 0 : split(value, values, /[[:space:]]+/)
        for (position = 1; position <= count; position++) { token = values[position]; if (token !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) invalid("invalid token"); if (seen[token]++) invalid("duplicate token"); if (token == required_token) found++ }
        next
      }
      if (line ~ (("(^|[^_[:alnum:]])" key "[[:space:]]*="))) invalid("malformed assignment")
    }
    END { if (!assignments) invalid("missing assignment"); if (found != 1) invalid("required token must occur exactly once"); exit(failed ? 1 : 0) }
  '
}
