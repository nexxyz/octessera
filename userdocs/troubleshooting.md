# Troubleshooting

Use the symptom below to choose the first safe action and the owner page. Keep
the board accessible while a physical issue is unresolved.

## Symptom router

| Symptom | First safe action | Continue with |
|---|---|---|
| OLED is blank, flickering, unstable, or shows two writers | Stop the test. Do not treat it as normal boot behavior. | [Raspberry first boot](hardware/raspberry-pi-first-boot.md), [Orange first boot](hardware/orange-pi-first-boot.md), [board qualification](hardware/board-qualification.md) |
| Setup hotspot or page does not appear | Keep the board powered only while following the setup retry path; do not interrupt an apply in progress. | [Setup portal](hardware/setup-portal.md), [Orange first boot](hardware/orange-pi-first-boot.md#if-setup-does-not-appear) |
| Setup applied partly or the hotspot vanished | Wait for the final result, reconnect with the new network or credentials, and start a new portal action for a retry. | [Setup portal](hardware/setup-portal.md#oled-modal-behaviour), [Orange security note](hardware/orange-pi-first-boot.md#security-note) |
| No sound from a board | Stop before changing wiring. Check the selected exact route, output connection, and board bring-up status. | [Board qualification](hardware/board-qualification.md), [matching first-boot page](README.md#1-choose-a-board) |
| USB audio or MIDI is missing | Treat the path as experimental/local bench validation, not public support. Use the selected build's host-data port only after its port-role, VBUS/CC, and no-backfeed gates pass; use a data-only or power-isolating cable when the instrument has separate power. | [Safety and power](hardware/safety-and-power.md#usb-host-connections), [release support](release-support.md#usb-policy), [pinout and connections](hardware/pinout-and-connections.md#usb-audio-and-midi) |
| Controls or grid respond incorrectly | Check the OLED for an overlay or Play page, then leave it with Back or navigate with Fn. Do not translate Raspberry pins to Orange. | [Controls cheat sheet](controls-cheat-sheet.md), [pinout and connections](hardware/pinout-and-connections.md), [board qualification](hardware/board-qualification.md) |
| Brownout, reboot, heat, or power instability | Power down. Return to the enclosure USB-C input and check the supply rating and host-cable isolation. | [Safety and power](hardware/safety-and-power.md), [assembly manual](hardware/assembly-manual.md#bench-bring-up-before-enclosure), [enclosure notes](hardware/enclosure.md#power-rule) |
| Orange `syslog` or `kern.log` fills with repeated `sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin` | Power down and inspect the HDMI plug and cable; then verify the exact Orange rsyslog drop-in. | [Orange production reference](../hardware/docs/orange-pi-production-reference.md#hdmi-plug-event-log-mitigation) |
| A sample is missing | Confirm which path you are using: the desktop package and both image constructors stage the 320-row artifact inventory, of which 318 WAV rows are sampler-loadable; two AIFF rows are attribution metadata outside the WAV-only browser/decoder. User samples remain supported. | [Desktop simulator](desktop-simulator.md#make-a-first-sound), [samples and OLED SD storage](README.md#samples-and-oled-sd-storage), [Raspberry first boot](hardware/raspberry-pi-first-boot.md), [Orange samples](hardware/orange-pi-first-boot.md#samples-and-output-paths) |
| The case does not fit or a port is blocked | Stop. Remove both microSD cards before fitting and find the interference instead of forcing the case. | [Enclosure notes](hardware/enclosure.md), [assembly enclosure steps](hardware/assembly-manual.md#enclosure-assembly) |
| Desktop simulator has no sound | Select the host's available audio output and start with a synth; the persisted audio toggles do not change the host's default endpoint. | [Desktop simulator](desktop-simulator.md#make-a-first-sound), [desktop limitations](desktop-simulator.md#what-this-path-can-and-cannot-tell-you) |

## When the page does not identify the fault

Stop at the first unclear physical gate. Record the board model, image, power
path, cable, and what the OLED or host showed. Then return to [board
qualification](hardware/board-qualification.md) and the relevant owner page
instead of trying a Raspberry procedure on Orange or a software workaround for
an electrical fault.

## Orange HDMI plug-event flood

A loose or intermittent HDMI plug caused a proven H618 plug-event storm that
filled Armbian's 50 MB RAM log. New Orange images drop only the exact repeated
kernel message before rsyslog writes `syslog` or `kern.log`; this is text-log
duplication mitigation, not a repair for a physical connector or cable fault.

From a console, use read-only checks:

```sh
sudo stat -c '%U:%G %a' /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf
sudo cat /etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf
sudo rsyslogd -N1 -f /etc/rsyslog.conf
sudo journalctl -k --no-pager
```

Journald's bounded diagnostic copy remains available. Do not respond by changing
global kernel logging, journald limits, or the RAM-log size. Power down before
reseating the HDMI plug or replacing the cable; if the exact event continues,
record the physical connector result and the preserved kernel journal.
