#!/bin/bash
set -euo pipefail

: "${ROOTFS_DIR:?pi-gen did not provide ROOTFS_DIR}"

rename_user_conf="$ROOTFS_DIR/etc/ssh/sshd_config.d/rename_user.conf"
userconfig_enablement="$ROOTFS_DIR/etc/systemd/system/multi-user.target.wants/userconfig.service"
userconfig_override="$ROOTFS_DIR/etc/systemd/system/userconfig.service.d"
userconfig_service="$ROOTFS_DIR/etc/systemd/system/userconfig.service"

rm -f "$rename_user_conf" "$userconfig_enablement"
rm -rf "$userconfig_override"
rm -f "$userconfig_service"
ln -s /dev/null "$userconfig_service"

test ! -e "$rename_user_conf" && test ! -L "$rename_user_conf"
test ! -e "$userconfig_enablement" && test ! -L "$userconfig_enablement"
test ! -e "$userconfig_override" && test ! -L "$userconfig_override"
test -L "$userconfig_service"
test "$(readlink "$userconfig_service")" = /dev/null
