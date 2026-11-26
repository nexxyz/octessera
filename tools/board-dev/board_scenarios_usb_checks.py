"""Checks for the scenarios driven from this host over the board's USB connection (host_usb_midi.py,
host_usb_audio.py): each reads the host's result JSON beside the journal window between its gates."""
import json
import re
import subprocess

from board_scenarios_evidence import journal, marks, window_counts, window_errors



def usb_midi(board, out):
    problems, evidence = [], []
    for phase, gate in (("straight", "usb_midi"), ("swing", "usb_midi_swing")):
        result = json.loads((out / f"usb-midi-{phase}.json").read_text())
        underruns, late, worst, journal_errors = window_counts(out, f"{gate}_ready", f"{gate}_go")
        problems += [f"{phase}: {p}" for p in result["problems"] + journal_errors]
        if underruns or late:
            problems.append(f"{phase}: {underruns} underruns and {late} late timed events (timing smoke: 0 and 0)")
        evidence.append(f"{phase} {json.dumps(result['metrics'])}, {underruns} underruns, {late} late timed events, "
                        f"worst runtime_late {worst}us")
    return problems, "; ".join(evidence)


def host_midi(out, scenario):
    result = json.loads((out / f"usb-midi-{scenario.removeprefix('usb_').removeprefix('midi_')}.json").read_text())
    underruns, late, worst, journal_errors = window_counts(out, f"{scenario}_ready", f"{scenario}_go")
    problems = result["problems"] + journal_errors
    if underruns or late:
        problems.append(f"{underruns} underruns and {late} late timed events (timing smoke: 0 and 0)")
    return problems, (f"{json.dumps(result['metrics'])}, {underruns} underruns, {late} late timed events, "
                      f"worst runtime_late {worst}us")


def usb_midi_cc(board, out):
    return host_midi(out, "usb_midi_cc")


def usb_notes_in(board, out):
    return host_midi(out, "usb_notes_in")


def weighted(stats):
    """avg and max over (n, avg, max) samples, the avg weighted by n."""
    total = sum(n for n, _, _ in stats)
    return (round(sum(n * avg for n, avg, _ in stats) / total) if total else None,
            max((top for _, _, top in stats), default=None))


def tempo_mod(board, out):
    """The host's clock-out checks, one pulse per runtime dispatch at most, and the saved BPM: the
    last CC's held value, not an LFO value. The peak tempo is 240 BPM (96 pulses/s, one every
    10.4 ms) and the runtime dispatches every 8 ms, so a second pulse in one dispatch means the
    runtime fell behind; the timing smoke baseline is 1."""
    problems, evidence = host_midi(out, "tempo_mod")
    found = marks(out)
    rows = [text for t, _, text in journal(out / "journal.log")
            if found["tempo_mod_ready"] <= t <= found["tempo_mod_go"]]
    timing = [m for text in rows for m in [re.search(
        r"runtime-timing playing .* max_pulses=(\d+) .* dispatch=\[n=(\d+) avg_us=(\d+) max_us=(\d+)\]", text)] if m]
    late = [m for text in rows for m in [re.search(r"runtime_late=n=(\d+) avg=(\d+)us max=(\d+)us", text)] if m]
    max_pulses = max((int(m.group(1)) for m in timing), default=None)
    dispatch = weighted([(int(m.group(2)), int(m.group(3)), int(m.group(4))) for m in timing])
    lateness = weighted([(int(m.group(1)), int(m.group(2)), int(m.group(3))) for m in late])
    held = json.loads((out / "usb-midi-tempo_mod.json").read_text())["metrics"]["held_bpm"]
    saved = json.loads((out / "tempo-patch.json").read_text())["runtimeConfig"]["transport"]["bpm"]
    if max_pulses is None or max_pulses > 1:
        problems.append(f"max_pulses per dispatch {max_pulses}, not 1")
    if abs(saved - held) > 1:
        problems.append(f"the saved patch plays BPM {saved}, not the last CC's {held}")
    return problems, (f"{evidence}; max_pulses {max_pulses}; dispatch avg/max {dispatch[0]}/{dispatch[1]} us; "
                      f"runtime_late avg/max {lateness[0]}/{lateness[1]} us; saved BPM {saved} (held {held})")


def usb_looper(board, out):
    """The host's timing checks, plus the autosaved steps: one press and one release per tapped tile
    (note 60 + tile), each press on the step it must land on."""
    problems, evidence = host_midi(out, "usb_looper")
    layer = json.loads((out / "looper-patch.json").read_text())["runtimeConfig"]["layers"][0]
    steps = layer["build"]["savedState"]["steps"]
    found = {}
    for index, events in enumerate(steps):
        for event in events:
            found.setdefault(event["tile"], []).append((event["kind"], index))
    expected = {0: 0, 1: 2, 2: 5, 3: 0}
    for tile, step in expected.items():
        kinds = sorted(kind for kind, _ in found.get(tile, []))
        if kinds != ["press", "release"] or (("press", step) not in found[tile]):
            problems.append(f"tile {tile}: saved {found.get(tile)}, not one press on step {step} and one release")
    if set(found) - set(expected):
        problems.append(f"unexpected tiles saved: {sorted(set(found) - set(expected))}")
    return problems, f"{evidence}; saved {found}"


def usb_audio_fx(board, out):
    result = json.loads((out / "usb-audio-fx.json").read_text())
    underruns, _, worst, _ = window_counts(out, "fx_hold", "fx_release")
    problems = result["problems"] + window_errors(out, "usb_audio_fx")
    if underruns:
        problems.append(f"{underruns} underruns while the tape stop was held")
    return problems, f"{json.dumps(result['metrics'])}, {underruns} underruns, worst runtime_late {worst}us"


def clip_frames(avi):
    probe = json.loads(subprocess.run(["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(avi)],
                                      capture_output=True, check=True).stdout)
    video = next(s for s in probe["streams"] if s["codec_type"] == "video")
    width, height = int(video["width"]), int(video["height"])
    rgb = subprocess.run(["ffmpeg", "-v", "error", "-i", str(avi), "-map", "0:v", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                         capture_output=True, check=True).stdout
    size = width * height * 3
    red = []
    for start in range(0, len(rgb) - size + 1, size):
        hits = 0
        for y in range(0, 5):
            row = rgb[start + (y * width + 88) * 3:start + (y * width + 104) * 3]
            hits += sum(1 for i in range(0, len(row), 3) if row[i] > 128 and row[i + 1] < 64 and row[i + 2] < 64)
        red.append(hits > 0)
    return red


def usb_audio(board, out):
    problems, evidence = [], []
    for take in ("level", "clip"):
        result = json.loads((out / f"usb-audio-{take}.json").read_text())
        problems += [f"{take}: {p}" for p in result["problems"]]
        evidence.append(f"{take} {json.dumps(result['metrics'])}")
    underruns, late, worst, journal_errors = window_counts(out, "usb_audio_ready", "usb_audio_go")
    problems += journal_errors
    if underruns:
        problems.append(f"{underruns} underruns during the level take")
    red = clip_frames(out / "clip.avi")
    pcm = subprocess.run(["ffmpeg", "-v", "error", "-i", str(out / "clip.avi"), "-map", "0:a", "-f", "s16le", "-ac", "1", "-"],
                         capture_output=True, check=True).stdout
    samples = memoryview(pcm).cast("h")
    clipped = [i for i in range(0, len(samples), 64) if max(map(abs, samples[i:i + 64])) >= 32767]
    last = clipped[-1] / 44100 if clipped else 0.0
    lit = sum(red[: int(last * 20)])
    cleared = next((i / 20 - last for i in range(int(last * 20), len(red)) if not any(red[i:i + 20])), None)
    if not clipped or lit < (last - 1) * 20 * 0.9:
        problems.append(f"CLIP lit in only {lit} of the {int(last * 20)} frames while the take clipped")
    if cleared is None or not 2.0 <= cleared <= 3.0:
        problems.append(f"CLIP cleared {cleared} s after the last clipped sample, not 2-3 s")
    evidence.append(f"{underruns} underruns, {late} late timed events, worst runtime_late {worst}us; recording clipped "
                    f"for {last:.2f} s, CLIP lit in {lit} of those frames, cleared "
                    f"{cleared if cleared is None else round(cleared, 2)} s after the last clipped sample")
    return problems, "; ".join(evidence)


def usb_hold_trig(board, out):
    return host_midi(out, "usb_hold_trig")


def usb_two_layers(board, out):
    """Each selection's own checks, plus one pass pattern whichever layer is selected."""
    problems, evidence, patterns = [], [], {}
    for selected in ("l1", "l2"):
        more, shown = host_midi(out, f"usb_two_layers_{selected}")
        problems += [f"{selected}: {p}" for p in more]
        evidence.append(f"{selected} {shown}")
        patterns[selected] = json.loads((out / f"usb-midi-two_layers_{selected}.json").read_text())["metrics"].get("pattern")
    if patterns["l1"] != patterns["l2"]:
        problems.append(f"the pass plays {patterns['l1']} with L1 selected but {patterns['l2']} with L2 selected")
    return problems, "; ".join(evidence)


# The board marks .go up to 1 s after the host touched it, the host notes the gate open up to about 1 s
# after the touch, and the last tile's note ends at its next scan step, up to one 2 s pass of 8 1/8 steps.
HOLD_MARK_SLACK_S, HOLD_RELEASE_S = 1.0, 2.0


def usb_audio_hold(board, out):
    """The take's own checks, plus: no quiet block or exact-zero run before the taps, and the sound stopping
    where the board's taps turned the tiles dark. The board marks give the taps' offset from the gate, the
    host the capture start's offset from it."""
    missing = [name for name in ("usb-audio-hold.json", "usb-audio-hold.opened") if not (out / name).exists()]
    if missing:
        return [f"no {' or '.join(missing)}: the host capture did not run or did not finish"], ""
    result = json.loads((out / "usb-audio-hold.json").read_text())
    metrics = result["metrics"]
    found = marks(out)
    opened = float((out / "usb-audio-hold.opened").read_text())
    take_start = metrics["capture_started"] + metrics["startup_s"] - opened
    taps = found["hold_taps"] - found["usb_audio_hold_go"] - take_start
    done = found["hold_taps_done"] - found["usb_audio_hold_go"] - take_start
    window = (round(taps - HOLD_MARK_SLACK_S, 2), round(done + HOLD_RELEASE_S + HOLD_MARK_SLACK_S, 2))
    underruns, _, worst, _ = window_counts(out, "usb_audio_hold_go", "usb_audio_hold_end")
    problems = result["problems"] + window_errors(out, "usb_audio_hold")
    early = [t for t in metrics["quiet_block_s"] + metrics["zero_run_s"] if t < window[0]]
    if early:
        problems.append(f"the held Sampler broke off or ran exact zero at {early[:6]} s, before the taps at "
                        f"{taps:.2f} s")
    last = metrics["last_sound_s"]
    if last is None or not window[0] <= last <= window[1]:
        problems.append(f"the sound stopped at {last} s into the take, not between {window[0]} and {window[1]} s "
                        f"where the taps turned the tiles dark")
    if underruns:
        problems.append(f"{underruns} underruns while the held Sampler played")
    return problems, (f"{json.dumps(metrics)}, taps at {taps:.2f}-{done:.2f} s into the take, {underruns} underruns, "
                      f"worst runtime_late {worst}us")


def usb_echo_repeat(board, out):
    problems, evidence = [], []
    for setup in ("usb_echo_midi", "usb_echo_synth"):
        more, shown = host_midi(out, setup)
        problems += [f"{setup}: {p}" for p in more]
        evidence.append(f"{setup} {shown}")
    return problems, "; ".join(evidence)
