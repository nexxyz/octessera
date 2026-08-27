# FAT board end-to-end paths

These are the two broad first-pass paths in the [FAT
orchestrator](fat-quick-run.md). Use the exact flashed release asset and
assembled board. The [diagnostic harness](fat-diagnostic-harness.md) is
additional sanitized evidence; it does not replace these operator observations.

Normal first boot does not wait six minutes. Upstream filesystem resize is ordered
before runtime; live Orange evidence saw resize complete from about 19.4 s to
22.7 s, with runtime starting at 24.3 s. Six minutes is only the upstream maximum
timeout, not a planned user-visible duration. The normal animated splash covers
this boot work.

On either board, if native ownership has not arrived by the 30-second handoff
window, the existing splash owner should show one static polished `STARTUP
DELAYED` / `PLEASE WAIT` frame and continue single-writer handoff polling. This
is delayed-start legibility/recovery mitigation, not a claim about resize state.
Only genuine errors or termination signals should produce black/off failure
cleanup.

For setup, the browser submission is provisional; the OLED terminal result is
authoritative. Success requires NetworkManager connected with a usable `wlan0`
IPv4 address, not Internet access, a default route, DNS, or ICMP. After success,
use `System > Info` for the `wlan0` IP; no reboot is required. `System > Updates`
is runtime-only Check/Apply/Rollback when Internet is available, not a full
OS/image, kernel, or device-tree update path.

These end-to-end setup steps keep the setup AP qualification separate. Use the
standalone `System > Backup & Restore` action in the gap stage for data transfer.

## Raspberry end-to-end

**Time box: 00:15–00:50.** Complete setup below through normal-WLAN network
access and SSH, then invoke the Raspberry block in the [diagnostic harness
evidence procedure](fat-diagnostic-harness.md#evidence-safe-invocation) exactly
once. Save its fresh evidence under `raspberry/00-fat-diagnostic`. Do not run a
second harness invocation in this path or in a later stage.

Existing scripts that call `/usr/local/bin/octessera-pi --diagnostic` without a
profile still reach the same Raspberry collector, but that compatibility form
is deprecated. `OCTESSERA_PI_DIAGNOSTIC=1` is the equivalent compatibility
environment alias. Do not combine either alias with a profile or interactive
hardware-test flag, and do not use either to select Orange; provide the
explicit Orange profile instead.

Capture only allowlisted service status fields as
`raspberry/04-runtime-log.txt` with the existing fixed SSH transport. Do not
copy raw service logs into shared evidence. If raw service logs are needed for
debugging, keep them outside the shared evidence tree and review/redact them
before retaining any excerpt:

```powershell
.\tools\pi\with-pi-ssh.ps1 ssh pi@192.168.0.218 "systemctl show --no-pager octessera.service --property=ActiveState,SubState,NRestarts,MainPID,InvocationID,User,UnitFileState" | Tee-Object -FilePath (Join-Path $Evidence "raspberry\04-runtime-log.txt")
if ($LASTEXITCODE -ne 0) { throw "Could not capture Raspberry production service status" }
```

### Operator action

1. Flash the exact Raspberry asset with Raspberry Pi Imager. Insert the card and
   power only through the enclosure USB-C input.
2. Watch the OLED from power-on through the normal menu. At the instrument,
   choose `System > Configure WiFi` and confirm `Open Portal`.
3. Join `Octessera Setup` or `Octessera Setup <4-char suffix>`, open
   `http://192.168.42.1`, and apply a test Wi-Fi network, hostname, and SSH key
   if needed. Wait for the OLED result, then reconnect on the new network and
   confirm the normal-WLAN address and SSH login.
4. Invoke the Raspberry diagnostic block exactly once as described above.
5. At the instrument, turn and press the Main encoder, press Back and Space,
   press one lower-left grid cell, start the default patch, and make one small
   parameter change. Do not start a full 64-cell or LED orientation sweep here.
6. With safe volume and the selected DAC connected, trigger one known default
   patch sound. If a direct route check is needed, use the existing tone command:

   ```bash
   timeout 15 speaker-test -D hw:0,0 -c 2 -t sine -f 440 -l 1
   ```

### Operator observation and implicit coverage

Record `raspberry/02-boot-oled.jpg` or `.mp4`,
`raspberry/03-setup-portal.txt`, `raspberry/05-controls-audio.txt`, the
diagnostic evidence, and the runtime log. Mark this stage `PASS` only when all
listed observations are present. A safe diagnostic or desktop simulator cannot
replace operator observations.

| Observation | What it validates in this path |
|---|---|
| Exact card boots and reaches the menu | Boot image, service start, native handoff, and first runtime snapshot |
| OLED remains readable with one normal owner | OLED initialization and boot-to-native handoff |
| AP, page, apply, and network reconnect work | First-boot setup portal and network adapter path |
| Main/NeoKey/grid input changes the live UI | Basic controls, input routing, and runtime message handoff |
| Default patch produces the known sound on the selected DAC | Selected audio routing, audio device open, realtime audio, and runtime action |
| No service failure or restart loop appears in the capture | Service lifecycle and native runtime startup |

This board path covers its own boot, service, OLED, setup, basic controls,
selected-route audio, and native runtime. Do not repeat those checks in USB,
lifecycle, or closeout stages. It does not prove the full physical sweep, USB,
Backup/Restore, or exact image-constructor relationship.

## Orange end-to-end

**Time box: 00:50–01:25.** Complete setup below through normal-WLAN network
access and SSH, then invoke the Orange block in the [diagnostic harness evidence
procedure](fat-diagnostic-harness.md#evidence-safe-invocation) exactly once.
Save its fresh output under `orange/00-fat-diagnostic`. This profile-aware
diagnostic is read-only and leaves the production service active.

The pristine diagnostic-image/initial Orange bring-up utility is reserved for
that workflow. Never run it against a production image during FAT; its output is
not production-FAT evidence.

After the first sound, capture production service status as
`orange/04-runtime-log.txt`. Do not copy raw journal output or unsanitized
metadata into shared evidence:

```powershell
$OrangeTarget = "octessera@<normal-wlan-ip>"
$OrangeSshOptions = @("-o", "StrictHostKeyChecking=yes")
& ssh @OrangeSshOptions $OrangeTarget "systemctl show --no-pager octessera.service --property=ActiveState,SubState,NRestarts,MainPID,InvocationID,User,UnitFileState" | Tee-Object -FilePath (Join-Path $Evidence "orange\04-runtime-log.txt")
if ($LASTEXITCODE -ne 0) { throw "Could not capture Orange production service status" }
```

This is a production-service-active status capture; do not stop or restart
`octessera.service` for it.

### Operator action

1. Flash the exact Orange production image with the selected image flasher. Do
   not use the diagnostic image or a Raspberry asset.
2. Watch the OLED from power-on through the normal menu. At the instrument,
   choose `System > Configure WiFi` and confirm `Open Portal`.
3. Wait for `Octessera Setup` or `Octessera Setup <4-char suffix>`, join the
   setup hotspot, open `http://192.168.42.1`, and apply a test Wi-Fi network and
   SSH password so the attended diagnostic can authenticate `sudo`. Wait for the
   OLED result before reconnecting, then confirm the normal-WLAN address, SSH
   login, and attended sudo credential. Do not expect an automatic hotspot on a
   fresh Orange production image.
4. Invoke the Orange diagnostic block exactly once as described above.
5. Turn and press the Main encoder, press Back and Space, press one lower-left
   grid cell, start the default patch, and make one small parameter change.
6. With the selected Jack DAC connected and safe volume, trigger one known
   default patch sound. The documented selected Jack route is
   `hw:CARD=octesseradac,DEV=0`; use the existing short tone shape if needed:

   ```bash
   timeout 15 speaker-test -D hw:CARD=octesseradac,DEV=0 -c 2 -t sine -f 440 -l 1
   ```

### Operator observation and implicit coverage

Record the Orange diagnostic evidence, `orange/02-boot-oled.jpg` or `.mp4`,
`orange/03-setup-portal.txt`, `orange/05-controls-audio.txt`, and the runtime
log. Mark this stage `PASS` only when the boot, service, OLED, setup, controls,
selected-route audio, and native runtime observations are present.

| Observation | What it validates in this path |
|---|---|
| Recorded production-image boot path reaches the normal menu | Boot, service start, native handoff, and first runtime snapshot; exact flashed-card identity remains unproven |
| OLED, setup, controls, and one Jack-route sound work | Shared first-pass user path plus Orange selected-route audio and input handoff |
| Production-safe diagnostic reports the expected service account, service state, fixed device paths, route/readiness, and passive UDC state | Orange adapter, service account/state, required fixed device paths, route/readiness, and passive UDC state |
| No service failure or restart loop appears in the capture | Service lifecycle and native runtime startup |

Stop if the production-safe diagnostic reports an unexpected board/profile,
missing required device or route/readiness state. Stop the run for an unstable
OLED or unsafe power. Pinmux, wiring, and physical I2S/DAC identity remain
operator- and board-specific checks.

The Raspberry path already covered the shared native flow. This Orange path adds
the recorded production-image boot path plus Orange adapter, service-account/state,
required device paths, route/readiness, and passive UDC evidence from the
production-safe diagnostic; exact flashed-card identity remains unproven.
It does not require a second shared-runtime demonstration. Pinmux and wiring
remain board-specific operator evidence. This path does not prove host USB
behavior, the full physical sweep, or Backup/Restore.
