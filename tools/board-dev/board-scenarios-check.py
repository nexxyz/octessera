"""Evaluates the evidence board-scenarios.sh collected and prints one PASS/FAIL line per scenario.

usage: board-scenarios-check.py <rpi|opi> <result-dir> <scenario>...
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

from board_scenarios_evidence import errors, fields, first, journal, marks, window_counts, window_errors
# Dispatched by name through globals() in main().
from board_scenarios_store_checks import bad_stores, full_card, resets  # noqa: F401
from board_scenarios_usb_checks import (  # noqa: F401
    tempo_mod, usb_audio, usb_audio_fx, usb_hold_trig, usb_looper, usb_midi, usb_midi_cc, usb_notes_in)

def boot(board, out):
    host = (out / "boot-host.log").read_text()
    ready = json.loads((out / "ready.json").read_text())
    entries = [json.loads(line) for line in (out / "boot-service.json").read_text().splitlines()]
    nearest = min(entries, key=lambda e: abs(int(e["__REALTIME_TIMESTAMP"]) / 1000 - ready["ready_at_unix_ms"]))
    menu = (ready["ready_at_unix_ms"] * 1000 - int(nearest["__REALTIME_TIMESTAMP"]) + int(nearest["__MONOTONIC_TIMESTAMP"])) / 1e6
    rows = journal(out / "boot-journal.log")
    splash = first(rows, "Started octessera-boot-splash.service") or first(rows, "Started octessera-orange-boot-splash.service")
    service = first(rows, "Started octessera.service")
    audio = first(rows, "Jack audio callback scheduling qualified")
    pointers = {}
    for line in host.splitlines():
        match = re.match(r"pass(\d) (\S+):hw_ptr\s*:\s*(\d+)", line)
        if match:
            pointers.setdefault(match.group(2), []).append(int(match.group(3)))
    running = [p for p, v in pointers.items() if len(v) == 2 and v[1] > v[0]]
    failed = [u for u in (out / "failed-units.log").read_text().splitlines() if "octessera" in u]
    before, after = ((out / name).read_text().split() for name in ("live-hashes-before.log", "live-hashes.log"))
    problems = errors(rows) + [f"failed unit {u}" for u in failed]
    if len(after) < 4 or before != after:
        problems.append("the live store changed across the reboot")
    if f"pid {ready['pid']} " not in host or "service active" not in host:
        problems.append("readiness marker does not belong to the running service")
    if not running:
        problems.append("no playback stream is advancing")
    if None in (splash, service, audio):
        problems.append("missing boot markers")
    timing = f"splash {splash}s, service {service}s, audio {audio}s, menu-ready {menu:.2f}s (kernel start = 0)"
    return problems, f"{timing}; streams advancing {running}; " + re.search(r"exe (\w+)", host).group(0)


def save_restore(board, out):
    def values(phase):
        system = json.loads((out / f"save-{phase}-system.json").read_text())["runtimeConfig"]
        patch = json.loads((out / f"save-{phase}-patch.json").read_text())["runtimeConfig"]
        return {"grid": system["gridBrightness"], "button": system["buttonBrightness"],
                "numbers": system["numericDisplayMode"], "bpm": patch["transport"]["bpm"]}
    before, edited, restarted, runtime = (values(p) for p in ("before", "edited", "restarted", "runtime"))
    expected = dict(before, grid=before["grid"] + 10, button=before["button"] + 10, bpm=before["bpm"] + 15,
                    numbers="numbers" if before["numbers"] == "bar" else "bar")
    problems = []
    if edited != expected:
        problems.append(f"edits not saved as expected {expected}")
    if restarted != edited:
        problems.append("restart changed the saved values")
    if runtime != dict(edited, grid=edited["grid"] + 5, bpm=edited["bpm"] + 5):
        problems.append("the restarted runtime did not continue from the saved values")
    problems += window_errors(out, "save_restore")
    return problems, f"before {before}, saved {edited}, after restart {restarted}, after one more detent {runtime}"


def recording(board, out):
    avi = out / "recording.avi"
    if not avi.exists():
        return ["no recording came back"], ""
    found = marks(out)
    played = found["rec_stop"] - found["rec_start"]
    probe = json.loads(subprocess.run(["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(avi)],
                                      capture_output=True, check=True).stdout)
    video = next(s for s in probe["streams"] if s["codec_type"] == "video")
    audio = next(s for s in probe["streams"] if s["codec_type"] == "audio")
    gray = subprocess.run(["ffmpeg", "-v", "error", "-i", str(avi), "-map", "0:v", "-f", "rawvideo", "-pix_fmt", "gray", "-"],
                          capture_output=True, check=True).stdout
    size = int(video["width"]) * int(video["height"])
    frames = [gray[i:i + size] for i in range(0, len(gray), size)]
    pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", str(avi), "-map", "0:a", "-f", "s16le", "-ac", "1", "-"],
                         capture_output=True, check=True).stdout
    samples = memoryview(pcm).cast("h")
    seconds_audio = len(samples) / int(audio["sample_rate"])
    seconds_video = len(frames) / 20
    distinct = len({hashlib.sha1(f).digest() for f in frames})
    lit = sum(1 for f in frames if max(f) > 16)
    peak = max(abs(s) for s in samples) if samples else 0
    problems = []
    if not played - 1 <= seconds_video <= played + 2:
        problems.append(f"video length {seconds_video:.2f}s does not match the {played:.1f}s recorded")
    if abs(seconds_audio - seconds_video) > 0.1:
        problems.append("audio and video lengths differ")
    if distinct < 10 or lit < len(frames) * 0.9:
        problems.append("screen frames are static or blank")
    if peak < 300:
        problems.append("audio is silent")
    problems += window_errors(out, "recording")
    return problems, (f"{avi.stat().st_size} bytes, {len(frames)} frames = {seconds_video:.2f}s at 20 fps, audio "
                      f"{seconds_audio:.2f}s, recorded {played:.1f}s, {distinct} distinct and {lit} lit frames, audio peak {peak}")


def oled(board, out):
    found = marks(out)
    events = []
    for line in (out / "oled-trace.txt").read_text().splitlines():
        match = re.search(r"\s([\d.]+): spi_transfer_start: \S+ \S+ len=\d+ tx=\[([0-9a-f-]*)\]", line)
        if match:
            events.append((float(match.group(1)), match.group(2)))
    contrast = []
    for index, (t, data) in enumerate(events[:-1]):
        if data == "c7":
            contrast.append((t, int(events[index + 1][1], 16)))
    configured = json.loads((out / "oled-before-system.json").read_text())["runtimeConfig"]["displayBrightness"] - 1

    def contrast_at(t):
        return next((v for s, v in reversed(contrast) if s < t), None)

    def count(byte, start, end):
        return sum(1 for t, data in events if data == byte and start <= t < end)

    off = next((t for t, data in events if data == "ae" and t > found["oled_ready"]), None)
    problems = []
    resent = [v for t, v in contrast if found["oled_start"] < t < found["oled_ready"]]
    if not resent or contrast_at(found["oled_ready"]) != configured:
        problems.append(f"contrast after the restart is {contrast_at(found['oled_ready'])}, not {configured}")
    if off is None or not found["oled_ready"] + 2 < off < found["oled_asleep"]:
        problems.append("display did not go off on the sleep timer")
    frames_asleep = count("5c", off or 0, found["oled_wake"])
    if frames_asleep:
        problems.append(f"{frames_asleep} frames written while asleep")
    if not count("af", found["oled_wake"], found["oled_woke"]):
        problems.append("display did not come back on with input")
    steps = [v for t, v in contrast if found["oled_bright"] <= t < found["oled_done"]]
    if contrast_at(found["oled_bright"]) != configured or steps != list(range(configured + 1, min(15, configured + 4) + 1)):
        problems.append(f"brightness detents sent contrast {steps}")
    problems += window_errors(out, "oled")
    sleep = f"{off - found['oled_ready']:.1f}s after ready" if off else "never"
    return problems, (f"contrast {contrast_at(found['oled_ready'])} after restart (resent {resent}), display off {sleep}, "
                      f"{frames_asleep} frames asleep, on again after input, detents sent contrast {steps}")


def soak(board, out):
    found = marks(out)
    rows = [r for r in journal(out / "journal.log") if found["soak_start"] <= r[0] <= found["soak_end"]]
    underruns, late, worst, _ = window_counts(out, "soak_start", "soak_end")
    missed = sum("quantum missed" in text for _, _, text in rows)
    pulses = sum(int(m.group(1)) for _, _, text in rows for m in [re.search(r"runtime-timing playing \S+ pulses=(\d+)", text)] if m)
    samples = [line.split() for line in (out / "soak-proc.log").read_text().splitlines()]
    rss = [int(s[1]) for s in samples if len(s) == 3]
    fds = [int(s[2]) for s in samples if len(s) == 3]

    def grows(values, margin):
        tail = values[2:]
        return len(tail) > 2 and all(b >= a for a, b in zip(tail, tail[1:])) and tail[-1] - tail[0] > margin

    warm = rss[20:]
    rss_grows = len(warm) > 1 and (warm[-1] - warm[0] > 256 or (len(warm) > 2 and all(b > a for a, b in zip(warm, warm[1:]))))
    problems = []
    if underruns or late or missed:
        problems.append(f"{underruns} underruns, {late} late timed events, {missed} missed quanta")
    if not pulses:
        problems.append("transport was not playing")
    if len(rss) < 26:
        problems.append("soak too short to judge RSS after the 20-minute warm-up")
    if rss_grows or grows(fds, 4):
        problems.append("RSS grows after the 20-minute warm-up, or open fds grow steadily")
    problems += errors(rows, sandboxed=True)
    return problems, (f"{underruns} underruns, {late} late timed events, {missed} missed quanta, {pulses} pulses, worst "
                      f"runtime_late {worst}us, RSS kB {rss}, fds {fds}")


def keymap(board, out):
    patch = json.loads((out / "keymap-patch.json").read_text())["runtimeConfig"]
    tiles = patch["layers"][0]["build"]["savedState"]["tiles"]
    saved = {(i % 8, i // 8) for i, on in enumerate(tiles) if on}
    expected = {(x, 0) for x in range(8)} | {(0, y) for y in range(4, 8)} | {(6, 5)}
    problems = window_errors(out, "keymap")
    if saved != expected:
        problems.append(f"missing {sorted(expected - saved)}, unexpected {sorted(saved - expected)}")
    return problems, f"saved tiles {sorted(saved)}"


def presets(board, out):
    rows = {name: values for name, values in map(fields, (out / "presets.log").read_text().splitlines())}
    first = rows["initial"]
    a, ab = first["A"], first["AB"]
    files = {step: {k: v for k, v in row.items() if k not in ("bpm", "note", "matches", "message")} for step, row in rows.items()}
    saved = files.get("save_as", {}).get("AC")
    edited = str(int(first["bpm"]) + 15)
    expected = {"save_as": lambda f: set(f) == {"A", "AB", "AC"} and f["A"] == a and f["AB"] == ab,
                "edit": lambda f: rows["edit"]["bpm"] == edited and set(f) == {"A", "AB", "AC"},
                "load": lambda f: (rows["load"]["note"] == "301" and "A" in rows["load"]["matches"].split(",")
                                   and set(f) == {"A", "AB", "AC"}),
                "rename": lambda f: set(f) == {"A", "AB", "ACD"} and f["ACD"] == saved,
                "overwrite": lambda f: set(f) == {"AB", "ACD"} and f["AB"] == a,
                "delete": lambda f: set(f) == {"AB"} and f["AB"] == a}
    problems = [f"after {step}: {files.get(step)} (bpm {rows.get(step, {}).get('bpm')}, "
                f"matches {rows.get(step, {}).get('matches')})"
                for step, check in expected.items() if step not in files or not check(files[step])]
    problems += window_errors(out, "presets")
    return problems, "; ".join(f"{step} {files[step]}" for step in files)


def stop_save(board, out):
    def values(phase):
        system = json.loads((out / f"stop-{phase}-system.json").read_text())["runtimeConfig"]
        patch = json.loads((out / f"stop-{phase}-patch.json").read_text())["runtimeConfig"]
        return system["gridBrightness"], patch["transport"]["bpm"]
    (grid, bpm), saved = values("before"), values("saved")
    log = (out / "stop.log").read_text().splitlines()
    stops = dict(line.split(" stop_ms=") for line in log if " stop_ms=" in line)
    issued = float(next(line.split("issued=")[1] for line in log if line.startswith("stop_edit_edited issued=")))
    edited = marks(out)["stop_edit_edited"]
    stopping = next((t for t, ident, text in journal(out / "journal.log")
                     if t >= edited and ident.startswith("systemd") and "Stopping" in text), None)
    sigterm_ms = None if stopping is None else round((stopping - edited) * 1000)
    problems = []
    if saved != (grid + 10, bpm + 15):
        problems.append(f"the stop right after the edits saved Grid Bright and BPM {saved}, not {(grid + 10, bpm + 15)}")
    if sigterm_ms is None or sigterm_ms > 200:
        problems.append(f"systemd began the stop {sigterm_ms} ms after the last edit, not within 200 ms")
    problems += [f"{name} took {ms} ms to stop" for name, ms in stops.items() if int(ms) >= 12000]
    partial = next(line.split("=")[1] for line in log if line.startswith("partial="))
    if partial != "0":
        problems.append(f"{partial} recording(s) left as .partial.wav")
    media = out / "stop-recording.media"
    seconds = 0.0
    if media.exists():
        probe = json.loads(subprocess.run(["ffprobe", "-v", "error", "-show_format", "-of", "json", str(media)],
                                          capture_output=True, check=True).stdout)
        seconds = float(probe["format"]["duration"])
        decoded = subprocess.run(["ffmpeg", "-v", "error", "-i", str(media), "-f", "null", "-"], capture_output=True)
        if decoded.returncode or decoded.stderr.strip():
            problems.append(f"the recording does not decode cleanly: {decoded.stderr[:200]!r}")
    if seconds < 4:
        problems.append(f"the recording stopped by the service stop lasts {seconds:.2f} s, not at least 4 s")
    problems += window_errors(out, "stop_save", sandboxed=True)
    return problems, (f"saved Grid Bright and BPM {saved} (from {(grid, bpm)}), stop issued "
                      f"{round((issued - edited) * 1000)} ms and begun {sigterm_ms} ms after the last edit, "
                      f"stops {stops}, recording {seconds:.2f} s")


def fuzz(board, out):
    """No crash and no exit but the restart request (75): only a plain exit right after a power request
    the sandbox refused is the sandbox's doing. Error lines from the sandbox's refusals are excused;
    any other one fails. RSS is reported: it warms up for the soak's 20 minutes."""
    logged = dict(fields(line) for line in (out / "fuzz.log").read_text().splitlines())
    start, end, edit, restart = logged["start"], logged["end"], logged["edit"], logged["restart"]
    found = marks(out)
    rows = [r for r in journal(out / "journal.log") if found["fuzz_begin"] <= r[0] <= found["fuzz_typed"]]
    problems, refused = [], []
    for t, _, text in rows:
        exited = re.search(r"Main process exited, code=(\w+), status=(\d+)", text)
        if "power lifecycle failed" in text or "power request" in text:
            refused.append(t)
        if "panicked" in text or (exited and exited.groups() != ("exited", "75")
                                  and not (exited.group(1) == "exited" and any(t - 10 <= r <= t for r in refused))):
            problems.append(f"{t:.1f} {text}")
    problems += [line for line in errors(rows, sandboxed=True)
                 if not any(r - 1 <= float(line.split()[0]) <= r + 1 for r in refused)]
    if end["active"] != "active" or restart["ready"] != "yes":
        problems.append(f"the service is {end['active']} after the fuzz, and ready after a restart: {restart['ready']}")
    if edit["after"] != edit["expected"]:
        problems.append(f"the closing Grid Bright edit did not land ({edit['before']} -> {edit['after']}, not "
                        f"{edit['expected']}): the walk left the keyboard or the menu unusable")
    restarts = sum("Main process exited" in text for _, _, text in rows)
    return problems, (f"seed {start['seed']}, {restarts} app exits, {len(errors(rows, sandboxed=True))} error lines, RSS "
                      f"{start['rss_kb']} -> {end['rss_kb']} kB (pid {start['pid']} -> {end['pid']}), closing edit "
                      f"{edit['before']} -> {edit['after']}")


def show_mode(board, out):
    rows = dict(fields(line) for line in (out / "show.log").read_text().splitlines())
    problems = []
    if rows["started"]["wlan_blocked"] != "yes" or rows["second_started"]["wlan_blocked"] != "yes":
        problems.append("Start 48h did not block the radio")
    at45, at100, ended, after_end = rows["at45"], rows["at100"], rows["ended"], rows["after_end"]
    if at45["elapsed"] != "40" or at45["release_timers"] != "1" or at45["wlan_blocked"] != "yes":
        problems.append(f"at 45 s: banked {at45['elapsed']} s (not 40), {at45['release_timers']} release timers")
    if at100["length"] or at100["elapsed"] or at100["wlan_blocked"] != "no":
        problems.append(f"the 70 s show did not end by itself: {at100}")
    if board == "rpi" and at100["health_timer"] != "active":
        problems.append("the network-health timer did not come back after the show")
    failed = [line for line in (out / "show-journal.log").read_text().splitlines()
              if "octessera-show-hold" in line and re.search(r"(?i)\bfail", line)]
    problems += [f"journal: {line}" for line in failed]
    if ended["wlan_blocked"] != "no" or int(ended["after"].rstrip("s")) > 5:
        problems.append(f"End left the radio blocked for {ended['after']}")
    if after_end["release_timers"] != "0" or after_end["length"]:
        problems.append(f"after End: {after_end['release_timers']} release timers, length '{after_end['length']}'")
    problems += window_errors(out, "show_mode")
    return problems, (f"at 45 s banked {at45['elapsed']} s with {at45['release_timers']} release timer(s); at 100 s "
                      f"length '{at100['length']}', radio blocked {at100['wlan_blocked']}, health timer "
                      f"{at100['health_timer']}; End unblocked the radio after {ended['after']} and left "
                      f"{after_end['release_timers']} release timers")


UPDATE_STEPS = {"good": ("0", "0.9.91", "Now running v0.9.91"), "back": ("0", "0.9.90", "Now running v0.9.90"),
                "dies": ("1", "0.9.90", "v0.9.92 did not start; went back to v0.9.90"),
                "hangs": ("1", "0.9.90", "v0.9.93 did not start; went back to v0.9.90")}


EXTRA = "usr/local/sbin/octessera-probe-extra"
# What the scratch system root and the releases' set-aside root files must hold after each step.
ROOT_FILES = {"good": {"probe": "0.9.91", "extra": "yes", "replaced_0.9.90": "0.9.90", "added_0.9.90": EXTRA},
              "back": {"probe": "0.9.90", "extra": "no", "replaced_0.9.90": "-", "added_0.9.90": "-",
                       "replaced_0.9.91": "-", "added_0.9.91": "-"},
              "dies": {"probe": "0.9.90", "extra": "no", "added_0.9.90": "-"},
              "hangs": {"probe": "0.9.90", "extra": "no", "added_0.9.90": "-"}}


def update_fail(board, out):
    rows = dict(fields(line) for line in (out / "update.log").read_text().splitlines() if line.split()[0] in UPDATE_STEPS)
    problems, evidence = [], []
    for name, (rc, current, message) in UPDATE_STEPS.items():
        row = rows[name]
        if row["rc"] != rc or row["current"] != current or message not in row["message"]:
            problems.append(f"{name}: exit {row['rc']}, current v{row['current']}, said '{row['message']}'")
        if row["active"] != "active" or int(row["ready_after"].rstrip("s")) >= 120:
            problems.append(f"{name}: service {row['active']}, ready {row['ready_after']} after the switch")
        if row["store_unchanged"] != "yes":
            problems.append(f"{name}: the store changed")
        root_files = {key: row.get(key) for key in ROOT_FILES[name]}
        if root_files != ROOT_FILES[name]:
            problems.append(f"{name}: root files {root_files}, not {ROOT_FILES[name]}")
        evidence.append(f"{name} took {row['took']}, ready {row['ready_after']} later on v{row['current']}, "
                        f"probe {row.get('probe')}")
    if "restart_ready=yes" not in (out / "update.log").read_text().split():
        problems.append("the clone did not load after a restart on the release it ended on")
    return problems, "; ".join(evidence)


RESTORED = "service=active live_store_unchanged=yes dropin_left=no gadget_restored=yes clone_mounted=no"


def restore(board, out):
    lines = [line.removeprefix("restore ") for line in (out / "host.log").read_text().splitlines()
             if line.startswith("restore ")]
    launches = (out / "launches.log").read_text().splitlines()
    problems = [f"a payload left the board as: {line}" for line in lines if line != RESTORED]
    if len(lines) != len(launches):
        problems.append(f"{len(launches)} payload runs launched, {len(lines)} reported their restore")
    return problems, f"{len(lines)} of {len(launches)} payload run(s) restored"


# Cases that fail on a known product bug until its fix lands: XFAIL while they fail, XPASS (drop the
# entry) once they pass. Neither counts as FAIL.
EXPECTED_FAIL = {"usb_hold_trig": "review a493f27c item 3: a Hold Trig note never ends with Event Triggers off"}


def main():
    board, out, scenarios = sys.argv[1], Path(sys.argv[2]), sys.argv[3:]
    for name in scenarios:
        try:
            problems, evidence = globals()[name](board, out)
        except (OSError, KeyError, ValueError, StopIteration, AttributeError, subprocess.CalledProcessError) as error:
            problems, evidence = [f"evidence missing or unreadable: {error!r}"], ""
        verdict = "FAIL" if problems else "PASS"
        if name in EXPECTED_FAIL:
            verdict = "XFAIL" if problems else "XPASS"
            evidence = f"{evidence} (expected to fail: {EXPECTED_FAIL[name]})"
        print(f"{verdict} {name}: {evidence}")
        for problem in problems:
            print(f"  - {problem}")


main()
