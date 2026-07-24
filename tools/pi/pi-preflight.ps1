param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$BoardProfile = "raspberry-pi-zero-2w"
)

$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "board-profile.ps1")
Assert-RaspberryBoardProfile $BoardProfile

$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", $Target)

$remote = @'
set -u

failures=0

check() {
  label="$1"
  shift
  printf '\n== %s ==\n' "$label"
  if "$@"; then
    return 0
  fi
  failures=$((failures + 1))
  return 0
}

check "identity" sh -c 'hostname && uname -a && id pi'
check "board profile" sh -c 'grep -qx "OCTESSERA_BOARD_PROFILE_ID=raspberry-pi-zero-2w" /etc/octessera/board-profile.env && /usr/local/bin/octessera-pi --print-build-metadata | grep -q raspberry-pi-zero-2w'
check "boot config" sh -c 'grep -nE "audio|hifiberry|i2s|spi|i2c|uart" /boot/firmware/config.txt'
check "device nodes" sh -c 'ls -l /dev/i2c-1 /dev/spidev0.0'
check "i2c access" sh -c 'test -r /dev/i2c-1 -a -w /dev/i2c-1'
check "spi access" sh -c 'test -r /dev/spidev0.0 -a -w /dev/spidev0.0'
check "pinctrl gpio14/15" sh -c 'pinctrl get 14 | grep -q "GPIO14 = input" && pinctrl get 15 | grep -q "GPIO15 = input" && pinctrl get 14 && pinctrl get 15'
check "pinctrl i2s/card detect" sh -c 'pinctrl get 18 | grep -q PCM_CLK && pinctrl get 19 | grep -q PCM_FS && pinctrl get 20 | grep -q "GPIO20 = input" && pinctrl get 21 | grep -q PCM_DOUT && pinctrl get 18 && pinctrl get 19 && pinctrl get 20 && pinctrl get 21'
check "alsa devices" sh -c 'aplay -l | grep -E "snd_rpi_hifiberry|HifiBerry|pcm5102a"'
check "service" sh -c 'systemctl is-enabled octessera.service 2>/dev/null && systemctl status octessera.service --no-pager --lines=5 2>/dev/null || true'

printf '\n== summary ==\n'
if [ "$failures" -eq 0 ]; then
  echo "Pi preflight passed"
  exit 0
fi

echo "Pi preflight found $failures failing check(s)"
exit 1
'@

$remote | ssh @sshArgs "bash -s"
