#!/usr/bin/env bash

octessera_armbian_env_update() {
  local source_file="$1"
  local destination_file="$2"
  local user_token="$3"
  local i2c_token="$4"
  local extra_user_token="${5:-}"

  awk -v user_token="$user_token" -v extra_user_token="$extra_user_token" -v i2c_token="$i2c_token" '
    function invalid(message) {
      print "Invalid Armbian environment: " message > "/dev/stderr"
      failed = 1
    }
    function parse_tokens(key, value, target,    clean, count, position, token, seen_key) {
      if (value ~ /#/) {
        invalid(key " cannot contain comments")
        return 0
      }
      clean = value
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", clean)
      if (clean == "") {
        return 0
      }
      count = split(clean, token_values, /[[:space:]]+/)
      found = 0
      for (position = 1; position <= count; position++) {
        token = token_values[position]
        if (token !~ /^[A-Za-z0-9][A-Za-z0-9_.-]*$/) {
          invalid(key " contains an invalid token")
        }
        seen_key = key SUBSEP token
        if (seen[seen_key]++) {
          invalid(key " contains a duplicate token")
        }
        if (token == target) {
          found++
        }
        if (extra_target != "" && token == extra_target) {
          extra_found++
        }
      }
      if (found > 1) {
        invalid(key " contains the target token more than once")
      }
      if (extra_found > 1) {
        invalid(key " contains the additional target token more than once")
      }
      return found
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        if (line ~ /user_overlays[[:space:]]*=/ || line ~ /(^|[^_[:alnum:]])overlays[[:space:]]*=/) {
          invalid("commented overlay assignment")
        }
        print line
        next
      }
      if (line ~ /^user_overlays=/) {
        if (user_assignments++) {
          invalid("duplicate user_overlays assignment")
        }
        value = substr(line, length("user_overlays=") + 1)
        extra_target = extra_user_token
        user_found = parse_tokens("user_overlays", value, user_token)
        if (!user_found) {
          line = line " " user_token
        }
        if (extra_user_token != "" && !extra_found) {
          line = line " " extra_user_token
        }
        print line
        next
      }
      if (line ~ /user_overlays[[:space:]]*=/) {
        invalid("malformed user_overlays assignment")
      }
      if (line ~ /^overlays=/) {
        if (overlay_assignments++) {
          invalid("duplicate overlays assignment")
        }
        value = substr(line, length("overlays=") + 1)
        overlay_found = parse_tokens("overlays", value, i2c_token)
        if (!overlay_found) {
          line = line " " i2c_token
        }
        print line
        next
      }
      if (line ~ /(^|[^_[:alnum:]])overlays[[:space:]]*=/) {
        invalid("malformed overlays assignment")
      }
      print line
    }
    END {
      if (!user_assignments) {
        print "user_overlays=" user_token (extra_user_token == "" ? "" : " " extra_user_token)
      }
      if (!overlay_assignments) {
        print "overlays=" i2c_token
      }
      exit(failed ? 2 : 0)
    }
  ' "$source_file" > "$destination_file"
}
