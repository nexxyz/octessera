#!/usr/bin/env bash

EXPECTED_MODEL="OrangePi Zero 2W"
EXPECTED_BOARD="orangepizero2w"
EXPECTED_IMAGE_KIND="armbian"
EXPECTED_BOARD_PROFILE="orange-pi-zero-2w"
EXPECTED_RUNTIME_ENABLED="false"

require_exact_assignment() {
  local file_path="$1"
  local key="$2"
  local expected="$3"
  local label="$4"
  local reason

  if [ ! -r "$file_path" ]; then
    record_failure "$label evidence is missing: $file_path"
    return 0
  fi
  if ! reason="$(awk -F= -v expected_key="$key" -v expected_value="$expected" '
    function fail(message) {
      if (reason == "") {
        reason = message
      }
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        if (line ~ "(^|[^[:alnum:]_])" expected_key "[[:space:]]*=") {
          fail("contains a commented " expected_key " assignment")
        }
        next
      }
      if (line ~ ("^" expected_key "=")) {
        assignment_count++
        actual = substr(line, length(expected_key) + 2)
        next
      }
      if (line ~ ("(^|[^[:alnum:]_])" expected_key "[[:space:]]*=")) {
        fail("contains a malformed " expected_key " assignment")
      }
    }
    END {
      if (assignment_count == 0) {
        fail("is missing " expected_key)
      } else if (assignment_count > 1) {
        fail("contains duplicate " expected_key " assignments")
      } else if (actual != expected_value) {
        fail("requires " expected_key "=" expected_value " (found: " actual ")")
      }
      if (reason != "") {
        print reason
        exit 1
      }
    }
  ' "$file_path")"; then
    record_failure "$label $reason"
  fi
}

validate_artifact_metadata() {
  local metadata_path="$1"

  if [ ! -r "$metadata_path" ]; then
    record_failure "Orange artifact metadata evidence is missing: $metadata_path"
    return 0
  fi
  require_exact_assignment "$metadata_path" OCTESSERA_IMAGE_KIND "$EXPECTED_IMAGE_KIND" \
    "Orange artifact metadata"
  require_exact_assignment "$metadata_path" OCTESSERA_BOARD_PROFILE_ID "$EXPECTED_BOARD_PROFILE" \
    "Orange artifact metadata"
  require_exact_assignment "$metadata_path" OCTESSERA_RUNTIME_ENABLED_DEFAULT "$EXPECTED_RUNTIME_ENABLED" \
    "Orange artifact metadata"
}

validate_boot_config() {
  local file_path="$1"
  local errors

  if [ ! -r "$file_path" ]; then
    record_failure "Armbian boot config is missing: $file_path"
    return 0
  fi

  errors="$(awk '
    function invalid(message) {
      if (!reported[message]++) {
        errors[++error_count] = message
      }
    }
    function parse_tokens(key, value,    clean, count, position, token, seen_key) {
      if (value ~ /#/) {
        invalid(key " assignment contains a comment")
        return
      }
      clean = value
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", clean)
      if (clean == "") {
        return
      }
      count = split(clean, token_values, /[[:space:]]+/)
      for (position = 1; position <= count; position++) {
        token = token_values[position]
        if (token !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) {
          invalid(key " contains an invalid token")
        }
        seen_key = key SUBSEP token
        if (seen[seen_key]++) {
          invalid(key " contains a duplicate token")
        }
        if (token == target[key]) {
          target_count[key]++
        }
        if (token == "spidev1_0") {
          stock_spidev = 1
        }
      }
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        if (line ~ /(^|[^[:alnum:]_])user_overlays[[:space:]]*=/) {
          invalid("commented user_overlays assignment")
        } else if (line ~ /(^|[^[:alnum:]_])overlays[[:space:]]*=/) {
          invalid("commented overlays assignment")
        }
        next
      }
      if (line ~ /^user_overlays=/) {
        user_assignments++
        if (user_assignments > 1) {
          invalid("duplicate user_overlays assignment")
        }
        target["user_overlays"] = "octessera-h618-spi1-oled-sd2"
        parse_tokens("user_overlays", substr(line, length("user_overlays=") + 1))
        next
      }
      if (line ~ /(^|[^[:alnum:]_])user_overlays[[:space:]]*=/) {
        invalid("malformed user_overlays assignment")
        next
      }
      if (line ~ /^overlays=/) {
        overlay_assignments++
        if (overlay_assignments > 1) {
          invalid("duplicate overlays assignment")
        }
        target["overlays"] = "i2c1-pi"
        parse_tokens("overlays", substr(line, length("overlays=") + 1))
        next
      }
      if (line ~ /(^|[^[:alnum:]_])overlays[[:space:]]*=/) {
        invalid("malformed overlays assignment")
      }
    }
    END {
      if (user_assignments != 1) {
        invalid("user_overlays assignment must occur exactly once")
      }
      if (overlay_assignments != 1) {
        invalid("overlays assignment must occur exactly once")
      }
      if (target_count["user_overlays"] != 1) {
        invalid("missing required token: user_overlays=octessera-h618-spi1-oled-sd2")
      }
      if (target_count["overlays"] != 1) {
        invalid("missing required token: overlays=i2c1-pi")
      }
      if (stock_spidev) {
        invalid("stock spidev1_0 overlay is enabled")
      }
      for (position = 1; position <= error_count; position++) {
        print errors[position]
      }
      exit(error_count ? 1 : 0)
    }
  ' "$file_path" 2>/dev/null || true)"

  if [ -n "$errors" ]; then
    while IFS= read -r error; do
      [ -n "$error" ] || continue
      record_failure "Armbian boot config $error"
    done <<< "$errors"
  fi
}

validate_identity_paths() {
  local armbian_release_path="$1"
  local os_release_path="$2"
  local metadata_path="$3"
  local model_path="$4"
  local machine="$5"
  local boot_config_path="$6"

  require_exact_assignment "$armbian_release_path" BOARD "$EXPECTED_BOARD" "Armbian board identity"
  require_exact_assignment "$os_release_path" VERSION_CODENAME trixie "Armbian OS codename"
  validate_artifact_metadata "$metadata_path"
  if [ ! -r "$model_path" ]; then
    record_failure "device-tree model evidence is missing: $model_path"
  elif ! cmp -s "$model_path" <(printf '%s\0' "$EXPECTED_MODEL"); then
    record_failure "device-tree model is not exactly $EXPECTED_MODEL"
  fi
  if [ "$machine" != "aarch64" ]; then
    record_failure "machine architecture is not exactly aarch64 (found: ${machine:-missing})"
  fi
  validate_boot_config "$boot_config_path"
}
