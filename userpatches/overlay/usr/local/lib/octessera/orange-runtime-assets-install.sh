#!/usr/bin/env bash

octessera_validate_orange_runtime_assets() {
  local overlay_dir="$1"
  local asset
  for asset in \
    usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts \
    usr/local/share/octessera/device-tree/octessera-h618-input-routing.dts \
    usr/local/share/octessera/device-tree/octessera-ahub0-pcm5102.dts \
    etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf \
    etc/modules-load.d/octessera-orange-midi.conf \
    usr/local/sbin/octessera-sd-card \
    etc/systemd/system/octessera-orange-sd-card.service \
    etc/udev/rules.d/99-octessera-orange-sd-card.rules \
    etc/systemd/system/octessera-orange-storage-control.socket \
    etc/systemd/system/octessera-orange-storage-control@.service \
    usr/local/sbin/octessera-orange-storage-control \
    usr/local/sbin/octessera-orange-storage \
    usr/local/lib/octessera/octessera-sd-card-lib.sh \
    usr/local/lib/octessera/device_config.py; do
    [[ -f "$overlay_dir/$asset" && ! -L "$overlay_dir/$asset" ]] || {
      echo "Missing fixed Orange runtime asset: $asset." >&2
      return 1
    }
  done
}

octessera_validate_orange_rsyslog_configuration() {
  local validation_config
  local validation_status
  validation_config="$(mktemp /tmp/octessera-rsyslog-validation.XXXXXX)" || return 1
  if printf '%s\n' 'global(net.enableDNS="off")' 'include(file="/etc/rsyslog.conf")' > "$validation_config"; then
    if rsyslogd -N1 -f "$validation_config"; then
      validation_status=0
    else
      validation_status=$?
    fi
  else
    validation_status=$?
  fi
  if rm -f -- "$validation_config"; then
    :
  else
    echo "Unable to remove temporary rsyslog validation config: $validation_config." >&2
    return 1
  fi
  return "$validation_status"
}

octessera_install_orange_runtime_assets() {
  local overlay_dir="$1"
  install_overlay_file usr/local/sbin/octessera-orange-usb-gadget /usr/local/sbin/octessera-orange-usb-gadget 0755
  install_overlay_file usr/local/sbin/octessera-sd-card /usr/local/sbin/octessera-sd-card 0755
  install_overlay_file usr/local/lib/octessera/octessera-sd-card-lib.sh /usr/local/lib/octessera/octessera-sd-card-lib.sh 0644
  install_overlay_file usr/local/sbin/octessera-orange-storage /usr/local/sbin/octessera-orange-storage 0755
  install_overlay_file usr/local/sbin/octessera-orange-storage-control /usr/local/sbin/octessera-orange-storage-control 0755
  install_overlay_file etc/systemd/system/octessera-orange-sd-card.service /etc/systemd/system/octessera-orange-sd-card.service 0644
  install_overlay_file etc/udev/rules.d/99-octessera-orange-sd-card.rules /etc/udev/rules.d/99-octessera-orange-sd-card.rules 0644
  install_overlay_file etc/systemd/system/octessera-orange-storage-control.socket /etc/systemd/system/octessera-orange-storage-control.socket 0644
  install_overlay_file etc/systemd/system/octessera-orange-storage-control@.service /etc/systemd/system/octessera-orange-storage-control@.service 0644
  install_overlay_file etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf 0644
  if command -v rsyslogd >/dev/null 2>&1; then
    octessera_validate_orange_rsyslog_configuration
  fi
  install_overlay_file usr/local/lib/octessera/device_config.py /usr/local/lib/octessera/device_config.py 0644
  install_overlay_file usr/local/sbin/octessera-device-apply-reboot /usr/local/sbin/octessera-device-apply-reboot 0755
  install_overlay_file usr/local/sbin/octessera-orange-oled-logo /usr/local/sbin/octessera-orange-oled-logo 0755
  install_overlay_file usr/local/sbin/octessera-orange-oled-handoff.py /usr/local/sbin/octessera-orange-oled-handoff.py 0644
  install_overlay_file usr/local/sbin/octessera-orange-oled-lifecycle.py /usr/local/sbin/octessera-orange-oled-lifecycle.py 0644
  install_overlay_file usr/local/sbin/octessera-orange-oled-suspend /usr/local/sbin/octessera-orange-oled-suspend 0755
  install_overlay_file usr/local/sbin/octessera-provision-musical-default /usr/local/sbin/octessera-provision-musical-default 0755
  install_overlay_file usr/local/lib/octessera/orange-sample-assets.sh /usr/local/lib/octessera/orange-sample-assets.sh 0644
  install_overlay_file etc/modules-load.d/octessera-orange-midi.conf /etc/modules-load.d/octessera-orange-midi.conf 0644
  install_overlay_file etc/modules-load.d/octessera-orange-usb-gadget.conf /etc/modules-load.d/octessera-orange-usb-gadget.conf 0644
  install_overlay_file etc/systemd/system/octessera-orange-usb-gadget.service /etc/systemd/system/octessera-orange-usb-gadget.service 0644
  install_overlay_file etc/systemd/system/octessera-device-apply-reboot.socket /etc/systemd/system/octessera-device-apply-reboot.socket 0644
  install_overlay_file etc/systemd/system/octessera-device-apply-reboot@.service /etc/systemd/system/octessera-device-apply-reboot@.service 0644
  install_overlay_file etc/systemd/system/octessera-provision-musical-default.service /etc/systemd/system/octessera-provision-musical-default.service 0644
  install_overlay_file etc/initramfs-tools/hooks/octessera-orange-boot-splash /etc/initramfs-tools/hooks/octessera-orange-boot-splash 0755
  install_overlay_file etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash /etc/initramfs-tools/scripts/init-premount/octessera-orange-boot-splash 0755
  install_overlay_file etc/systemd/system/octessera-orange-boot-splash.service /etc/systemd/system/octessera-orange-boot-splash.service 0644
  install_overlay_file etc/systemd/system/octessera-orange-oled-shutdown.service /etc/systemd/system/octessera-orange-oled-shutdown.service 0644
  install_overlay_file etc/systemd/system/octessera-orange-oled-suspend.service /etc/systemd/system/octessera-orange-oled-suspend.service 0644
  install_overlay_file usr/local/share/octessera-setup-ui/img/octessera-mark.svg /usr/share/octessera/oled/octessera-mark.svg 0644
  install_overlay_file usr/local/share/octessera-setup-ui/img/octessera-wordmark.svg /usr/share/octessera/oled/octessera-wordmark.svg 0644
  install_overlay_file usr/local/share/octessera/oled/octessera-pi-booting.rgb565 /usr/share/octessera/oled/octessera-pi-booting.rgb565 0644
  install_overlay_file usr/local/share/octessera/oled/octessera-pi-shutdown.rgb565 /usr/share/octessera/oled/octessera-pi-shutdown.rgb565 0644
}

octessera_install_orange_production_assets() {
  local overlay_dir="$1"
  [[ -f "$overlay_dir/etc/udev/rules.d/70-octessera-orange-runtime.rules" && ! -L "$overlay_dir/etc/udev/rules.d/70-octessera-orange-runtime.rules" ]] || { echo "Missing exact Orange runtime udev rule." >&2; return 1; }
  install_overlay_file etc/udev/rules.d/70-octessera-orange-runtime.rules /etc/udev/rules.d/70-octessera-orange-runtime.rules 0644
  install_overlay_file etc/systemd/system/octessera.service /etc/systemd/system/octessera.service 0644
  octessera_install_production_runtime "$overlay_dir"
}

octessera_enable_orange_runtime_services() {
  systemctl enable octessera-orange-usb-gadget.service >/dev/null
  systemctl enable octessera-orange-sd-card.service >/dev/null
  systemctl enable octessera-orange-storage-control.socket >/dev/null
  systemctl enable octessera-device-apply-reboot.socket >/dev/null
  systemctl enable octessera-provision-musical-default.service >/dev/null
  systemctl enable octessera-orange-boot-splash.service >/dev/null
  systemctl enable octessera-orange-oled-shutdown.service >/dev/null
  systemctl enable octessera-orange-oled-suspend.service >/dev/null
}
