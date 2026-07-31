#!/usr/bin/env bash

octessera_remove_uart0_console_args() {
  local source_file="$1"
  local destination_file="$2"

  awk '
    function fail(message) {
      print "Invalid Armbian boot configuration: " message > "/dev/stderr"
      failed = 1
    }
    function emit_tokens(prefix, value,    count, position, token, output) {
      count = split(value, values, /[[:space:]]+/)
      output = ""
      for (position = 1; position <= count; position++) {
        token = values[position]
        if (token == "" || token ~ /^console=ttyS0(,|$)/) {
          continue
        }
        output = output (output == "" ? "" : " ") token
      }
      print prefix output
    }
    {
      line = $0
      if (line ~ /^[[:space:]]*#/) {
        print line
        next
      }
      if (line ~ /^[[:space:]]*(extraargs|bootargs)[[:space:]]*=/) {
        if (line !~ /^(extraargs|bootargs)=/) {
          fail("boot argument assignment must use its canonical key")
          next
        }
        key = substr(line, 1, index(line, "=") - 1)
        if (seen[key]++) {
          fail("duplicate " key " assignment")
          next
        }
        emit_tokens(key "=", substr(line, length(key) + 2))
        next
      }
      if (line ~ /(^|[^_[:alnum:]])(extraargs|bootargs)[[:space:]]*=/) {
        fail("malformed boot argument assignment")
        next
      }
      if (line ~ /^[[:space:]]*[Aa][Pp][Pp][Ee][Nn][Dd][[:space:]]+/) {
        match(line, /^[[:space:]]*[Aa][Pp][Pp][Ee][Nn][Dd][[:space:]]+/)
        prefix = substr(line, 1, RLENGTH)
        emit_tokens(prefix, substr(line, length(prefix) + 1))
        next
      }
      print line
    }
    END {
      exit(failed ? 2 : 0)
    }
  ' "$source_file" > "$destination_file"
}

octessera_assert_no_uart0_console_args() {
  local config_file="$1"
  awk '
    /^[[:space:]]*#/ { next }
    {
      line = $0
      if (line ~ /(^|[[:space:]])console=ttyS0(,|$)/) {
        found = 1
      }
    }
    END { exit(found ? 1 : 0) }
  ' "$config_file"
}
