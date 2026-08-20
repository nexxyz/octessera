#!/usr/bin/env bash
# shellcheck disable=SC2154

octessera_require_runtime_udev_rule() {
  local rule_path=etc/udev/rules.d/70-octessera-orange-runtime.rules expected_rule rule_content metadata
  if [[ -d "$target" ]]; then [[ -f "$target/$rule_path" && ! -L "$target/$rule_path" ]] || { echo 'Orange runtime udev rule is not a regular file.' >&2; exit 1; }; else metadata="$(octessera_debugfs_stat_metadata "$target" "$rule_path")" || { echo 'Unable to inspect Orange runtime udev rule.' >&2; exit 1; }; [[ "$(octessera_debugfs_type "$metadata")" == regular ]] || { echo 'Orange runtime udev rule is not a regular file.' >&2; exit 1; }; fi
  require_root_mode "$rule_path" 644
  expected_rule=$'KERNEL=="i2c-2", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="spidev1.0", GROUP="octessera-runtime", MODE="0660"\nKERNEL=="gpiochip1", GROUP="octessera-runtime", MODE="0660"'
  rule_content="$(read_file "$rule_path")"
  [[ "$rule_content" == "$expected_rule" || "$rule_content" == "$expected_rule"$'\n' ]] || { echo 'Orange runtime udev rule content is not exact.' >&2; exit 1; }
}
