"""Closed-loop USB MIDI timing test against a board running the usb_midi scenario (run on the host).

usage: host_usb_midi.py <Raspberry|Orange> <straight|swing|cc> <result.json>
straight: Start, 24 PPQN clock at 100 BPM for 120 s, 140 BPM for 60 s, Stop. swing: 100 BPM for 30 s
with the board on Swing 4:5. cc: 100 BPM for 36 s with CC1 at 16, 48 and 96 for 12 s each; the board
maps CC1 to Vel Scale, so the mirrored Drum's velocity must follow each value from the next step on. Every incoming message is stamped with perf_counter while the same loop
sends the clock, so the stamps share the clock's time base. Intervals near a clock pulse the host
itself sent more than 1 ms late are left out of the interval checks and counted. Needs python-rtmidi.
"""
import ctypes
import json
import statistics
import sys
import time

import rtmidi

DRUM, MIDI_DRUM = 0, 9
TOLERANCE_MS = 2.0
HOST_LATE_S = 0.001
DURATION_TOLERANCE_MS = 3.0
CC_STEPS = [(0.0, 16), (12.0, 48), (24.0, 96)]
# One 1/8 step at 100 BPM plus a margin: a CC applies from the next step on.
CC_SETTLE_S = 0.35


def port(ports, name):
    return next(i for i, label in enumerate(ports) if name in label and "Octessera" in label)


def drive(name, segments, ccs=()):
    ctypes.windll.winmm.timeBeginPeriod(1)
    ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x80)
    ctypes.windll.kernel32.SetThreadPriority(ctypes.windll.kernel32.GetCurrentThread(), 15)
    midi_in, midi_out = rtmidi.MidiIn(), rtmidi.MidiOut()
    midi_in.ignore_types(sysex=True, timing=False, active_sense=True)
    midi_in.open_port(port(midi_in.get_ports(), name))
    midi_out.open_port(port(midi_out.get_ports(), name))
    received, ticks, changes, late, sent_ccs = [], [], [], [], []
    pending_ccs = list(ccs)

    def until(deadline):
        while (now := time.perf_counter()) < deadline:
            message = midi_in.get_message()
            if message:
                received.append((now, message[0]))

    start = time.perf_counter() + 0.5
    until(start)
    midi_out.send_message([0xFA])
    sent = time.perf_counter()
    due = sent
    for bpm, seconds in segments:
        changes.append((due, bpm))
        for _ in range(round(seconds * bpm * 24 / 60)):
            until(due)
            while pending_ccs and due - sent >= pending_ccs[0][0]:
                value = pending_ccs.pop(0)[1]
                midi_out.send_message([0xB0, 1, value])
                sent_ccs.append((time.perf_counter(), value))
            midi_out.send_message([0xF8])
            ticks.append(time.perf_counter())
            if ticks[-1] - due > HOST_LATE_S:
                late.append(ticks[-1])
            due += 60 / bpm / 24
    until(due)
    midi_out.send_message([0xFC])
    stop = time.perf_counter()
    until(stop + 3.0)
    midi_in.close_port()
    midi_out.close_port()
    return {"start": sent, "stop": stop, "ticks": ticks, "changes": changes, "received": received, "host_late": late,
            "ccs": sent_ccs}


def notes(received):
    """(channel, note, on, off, velocity) for every note-on, off None when it never ended."""
    held, found = {}, []
    for t, message in received:
        status = message[0] & 0xF0
        if status not in (0x80, 0x90):
            continue
        key = (message[0] & 0x0F, message[1])
        if status == 0x90 and message[2] > 0:
            entry = [key[0], key[1], t, None, message[2]]
            found.append(entry)
            held.setdefault(key, []).append(entry)
        elif held.get(key):
            held[key].pop(0)[3] = t
    return found


def span(values):
    return {"n": len(values), "min": round(min(values), 2), "max": round(max(values), 2),
            "mean": round(statistics.fmean(values), 2)} if values else {"n": 0}


def analyze_cc(run):
    """Each CC value's window must play one velocity, and the velocities must rise with the values."""
    problems, windows = [], []
    drum = [n for n in notes(run["received"]) if n[0] == DRUM]
    bounds = [t for t, _ in run["ccs"]] + [run["stop"]]
    for (sent, value), end in zip(run["ccs"], bounds[1:]):
        velocities = sorted({n[4] for n in drum if sent + CC_SETTLE_S <= n[2] < end})
        windows.append({"cc": value, "velocities": velocities})
        if len(velocities) != 1:
            problems.append(f"CC1 {value}: Drum velocities {velocities} from one step after it, not one value")
    played = [w["velocities"][0] for w in windows if len(w["velocities"]) == 1]
    if len(played) == len(windows) and played != sorted(set(played)):
        problems.append(f"Drum velocities {played} do not rise with CC1 {[w['cc'] for w in windows]}")
    return problems, {"windows": windows, "ccs_sent": len(run["ccs"])}


def analyze(run, phase):
    if phase == "cc":
        return analyze_cc(run)
    problems, metrics = [], {}
    found = notes(run["received"])
    drum = [n for n in found if n[0] == DRUM]
    onsets = [n[2] for n in drum]
    if len(onsets) < 20:
        return [f"only {len(onsets)} Drum notes came back"], metrics
    clean = [not any(a - 0.05 <= t <= b for t in run["host_late"]) for a, b in zip(onsets, onsets[1:])]
    ms = [(b - a) * 1000 if ok else None for a, b, ok in zip(onsets, onsets[1:], clean)]
    metrics["host_late_pulses"] = len(run["host_late"])
    metrics["intervals_skipped_for_host"] = clean.count(False)
    bar = 60 / 100 * 4
    if phase == "straight":
        change = run["changes"][1][0]
        steady = [d for (a, b), d in zip(zip(onsets, onsets[1:]), ms) if d and run["start"] + bar <= a and b < change]
        metrics["ioi_100_ms"] = span(steady)
        if not steady or any(abs(d - 300) > TOLERANCE_MS for d in steady):
            problems.append(f"100 BPM inter-onset intervals {span(steady)} are not 300 +- {TOLERANCE_MS} ms")
        first = max(i for i, t in enumerate(run["ticks"]) if t <= onsets[0])
        latency = [(t - run["ticks"][first + 12 * i]) * 1000 for i, t in enumerate(onsets) if t < change]
        drift = statistics.median(latency[-10:]) - statistics.median(latency[:10])
        metrics["latency_ms"] = span(latency)
        metrics["drift_ms"] = round(drift, 2)
        if abs(drift) >= TOLERANCE_MS:
            problems.append(f"accumulated drift {drift:.2f} ms over the 100 BPM segment")
        follow = [d for (a, b), d in zip(zip(onsets, onsets[1:]), ms) if d and a >= change + 60 / 140 * 4 and b < run["stop"]]
        metrics["ioi_140_ms"] = span(follow)
        if not follow or any(abs(d - 60000 / 140 / 2) > TOLERANCE_MS for d in follow):
            problems.append(f"140 BPM intervals one bar after the change {span(follow)} are not 214.29 +- {TOLERANCE_MS} ms")
    else:
        pairs = [d for a, d in zip(onsets, ms) if a >= run["start"] + bar]
        long_short = [(pairs[i], pairs[i + 1]) for i in range(len(pairs) - 1)
                      if pairs[i] and pairs[i + 1] and pairs[i] < pairs[i + 1]]
        metrics["swing_on_to_off_ms"] = span([a for a, _ in long_short])
        metrics["swing_off_to_on_ms"] = span([b for _, b in long_short])
        # 4:5 puts the offbeat at 4/9 of the beat, 10.67 pulses: external sync plays it on pulse 11.
        if len(long_short) < 10 or any(abs(a - 275) > TOLERANCE_MS or abs(b - 325) > TOLERANCE_MS for a, b in long_short):
            problems.append(f"swung intervals {metrics['swing_on_to_off_ms']} / {metrics['swing_off_to_on_ms']} "
                            f"are not 275 / 325 ms (offbeat on pulse 11)")
    for channel, expected, label in ((DRUM, 100, "mirrored Drum"), (MIDI_DRUM, 150, "MIDI Drum")):
        lengths = [(n[3] - n[2]) * 1000 for n in found if n[0] == channel and n[3] is not None
                   and not any(n[2] - 0.05 <= t <= n[3] for t in run["host_late"])]
        metrics[f"{label.replace(' ', '_').lower()}_length_ms"] = span(lengths)
        if not lengths or any(abs(d - expected) > DURATION_TOLERANCE_MS for d in lengths):
            problems.append(f"{label} note lengths {span(lengths)} are not {expected} ms")
    unpaired = [n for n in found if n[3] is None]
    late_on = [n for n in found if n[2] > run["stop"] + 0.05]
    after = [t for t, _ in run["received"] if t > run["stop"] + 0.3]
    metrics["after_stop"] = {"unpaired": len(unpaired), "late_note_ons": len(late_on), "messages_after_300ms": len(after)}
    if unpaired or late_on or after:
        problems.append(f"after Stop: {len(unpaired)} unpaired note-ons, {len(late_on)} note-ons later than 50 ms, "
                        f"{len(after)} messages in the following 2.7 s")
    metrics["notes"] = {"drum": len(drum), "midi_drum": sum(n[0] == MIDI_DRUM for n in found)}
    return problems, metrics


def main():
    name, phase, result = sys.argv[1:4]
    segments = {"straight": [(100, 120), (140, 60)], "swing": [(100, 30)], "cc": [(100, 36)]}[phase]
    run = drive(name, segments, CC_STEPS if phase == "cc" else ())
    problems, metrics = analyze(run, phase)
    with open(result, "w") as out:
        json.dump({"phase": phase, "problems": problems, "metrics": metrics, "run": run}, out)
    print(f"{'FAIL' if problems else 'PASS'} usb_midi {phase}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


main()
