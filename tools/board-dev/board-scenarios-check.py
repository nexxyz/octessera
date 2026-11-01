"""Evaluates the evidence board-scenarios.sh collected and prints one PASS/FAIL line per scenario.

usage: board-scenarios-check.py <rpi|opi> <result-dir> <scenario>...
"""
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ERROR = re.compile(r"(?i)\b(fail\w*|error|panicked|unavailable)\b")
BENIGN = ("HDMI framebuffer waiting",)
JOURNAL = re.compile(r"^\[\s*([\d.]+)\] \S+ ([^\[:]+)(?:\[\d+\])?: (.*)$")


def journal(path):
    rows = []
    for line in Path(path).read_text(errors="replace").splitlines():
        match = JOURNAL.match(line)
        if match:
            rows.append((float(match.group(1)), match.group(2), match.group(3)))
    return rows


def errors(rows, start=0.0, end=float("inf")):
    return [f"{t:.1f} {ident}: {text}" for t, ident, text in rows
            if start <= t <= end and ident.startswith(("octessera", "systemd"))
            and ("octessera" in ident or "octessera" in text)
            and ERROR.search(text) and not any(b in text for b in BENIGN)]


def marks(out):
    found = {}
    for line in (out / "marks.log").read_text().splitlines():
        t, name = line.split(" ", 1)
        found.setdefault(name.split(" ")[0], float(t))
    return found


def window_errors(out, name):
    found = marks(out)
    return errors(journal(out / "journal.log"), found[f"{name}_begin"], found[f"{name}_end"])


def first(rows, needle, ident=None):
    return next((t for t, who, text in rows if needle in text and (ident is None or who == ident)), None)


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
    if len(after) != 6 or before != after:
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
    underruns = sum("underrun occurred" in text for _, _, text in rows)
    missed = sum("quantum missed" in text for _, _, text in rows)
    late = sum(int(m.group(1)) for _, _, text in rows for m in [re.search(r"late timed events: (\d+)", text)] if m)
    pulses = sum(int(m.group(1)) for _, _, text in rows for m in [re.search(r"runtime-timing playing \S+ pulses=(\d+)", text)] if m)
    worst = max((int(m.group(1)) for _, _, text in rows for m in [re.search(r"runtime_late=n=\d+ avg=\d+us max=(\d+)us", text)] if m), default=None)
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
    problems += errors(rows)
    return problems, (f"{underruns} underruns, {late} late timed events, {missed} missed quanta, {pulses} pulses, worst "
                      f"runtime_late {worst}us, RSS kB {rss}, fds {fds}")


def main():
    board, out, scenarios = sys.argv[1], Path(sys.argv[2]), sys.argv[3:]
    for name in scenarios:
        try:
            problems, evidence = globals()[name](board, out)
        except (OSError, KeyError, ValueError, StopIteration, AttributeError, subprocess.CalledProcessError) as error:
            problems, evidence = [f"evidence missing or unreadable: {error!r}"], ""
        verdict = "FAIL" if problems else "PASS"
        print(f"{verdict} {name}: {evidence}")
        for problem in problems:
            print(f"  - {problem}")


main()
