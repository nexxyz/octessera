param(
  [string]$Target = "pi@192.168.0.211",
  [string]$Key = "$env:USERPROFILE\.ssh\octessera_pi_dev",
  [string]$OutputDir = "diagnostics"
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path -LiteralPath $OutputDir)) {
  New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$hostName = $Target.Split("@")[1]
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
$safeTarget = $Target -replace "[^A-Za-z0-9_.-]", "_"
$outputPath = Join-Path $OutputDir "pi-network-$safeTarget-$timestamp.txt"
$sshArgs = @("-i", $Key, "-o", "IdentitiesOnly=yes", "-o", "ConnectTimeout=10", $Target)

$remote = @'
set -u

echo "--- identity/current ---"
date -Is
hostname
whoami
uptime
who -b || true
last -x | head -35 || true
journalctl --list-boots --no-pager || true

echo "--- power/temp ---"
vcgencmd get_throttled 2>/dev/null || true
vcgencmd measure_temp 2>/dev/null || true
vcgencmd measure_volts 2>/dev/null || true

echo "--- network state ---"
/usr/sbin/iw dev wlan0 get power_save 2>/dev/null || true
/usr/sbin/iw dev wlan0 link 2>/dev/null || true
ip -brief addr || true
ip route || true
nmcli dev status 2>/dev/null || true
nmcli -f 802-11-wireless.powersave,connection.autoconnect,connection.autoconnect-retries,ipv4.method,ipv6.method connection show preconfigured 2>/dev/null || true

echo "--- service state ---"
systemctl is-enabled ssh NetworkManager wpa_supplicant octessera-network-health.timer octessera.service 2>/dev/null || true
systemctl is-active ssh NetworkManager wpa_supplicant octessera-network-health.timer octessera.service 2>/dev/null || true
systemctl --no-pager --full status ssh NetworkManager wpa_supplicant octessera-network-health.timer octessera.service 2>/dev/null | sed -n '1,180p' || true

echo "--- network health log ---"
tail -n 260 /var/log/octessera/network-health.log 2>/dev/null || true

echo "--- current boot warnings ---"
journalctl -b -p warning..alert --no-pager 2>/dev/null | tail -n 240 || true

echo "--- current boot network/ssh/wifi selected ---"
journalctl -b -o short-iso --no-pager 2>/dev/null | grep -Ei 'wlan0|wifi|brcm|brcmfmac|cfg80211|NetworkManager|wpa_supplicant|CTRL-EVENT|dhcp|carrier|deauth|disassoc|disconnect|rfkill|avahi|ssh|sshd|octessera-network-health|recovery=|power save|under-voltage|thrott|thermal|mmc|i/o error|watchdog|oom|panic|blocked|hung|firmware|bus is down|trap' | tail -n 420 || true

echo "--- previous boot network/ssh/wifi selected ---"
journalctl -b -1 -o short-iso --no-pager 2>/dev/null | grep -Ei 'wlan0|wifi|brcm|brcmfmac|cfg80211|NetworkManager|wpa_supplicant|CTRL-EVENT|dhcp|carrier|deauth|disassoc|disconnect|rfkill|avahi|ssh|sshd|octessera-network-health|recovery=|power save|under-voltage|thrott|thermal|mmc|i/o error|watchdog|oom|panic|blocked|hung|firmware|bus is down|trap' | tail -n 420 || true
'@

& {
  "--- local reachability ---"
  "timestamp=$(Get-Date -Format o)"
  "target=$Target"
  "ping=$((Test-Connection -ComputerName $hostName -Count 2 -Quiet))"
  Test-NetConnection -ComputerName $hostName -Port 22
  "--- remote diagnostics ---"
  $remote | ssh @sshArgs "tr -d '\r' | bash -s"
} 2>&1 | Tee-Object -FilePath $outputPath

if ($LASTEXITCODE -ne 0) {
  throw "diagnostic SSH command failed with exit code $LASTEXITCODE; partial output: $outputPath"
}

"wrote $outputPath"
