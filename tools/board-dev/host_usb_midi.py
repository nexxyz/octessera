"""Closed-loop USB MIDI timing test against a board running the usb_midi scenario (run on the host).

usage: host_usb_midi.py <Raspberry|Orange> <straight|swing|cc|looper|notes_in|hold_trig|tempo_mod> <result.json>
straight: Start, 24 PPQN clock at 100 BPM for 120 s, 140 BPM for 60 s, Stop. swing: 100 BPM for 30 s
with the board on Swing 4:5. cc: 100 BPM for 36 s with CC1 at 16, 48 and 96 for 12 s each; the board
maps CC1 to Vel Scale, so the mirrored Drum's velocity must follow each value from the next step on.
looper: 100 BPM for four passes of an 8-step 1/8 Looper, keyboard notes on channel 1 at Start, early
in step 3 and late in step 5, and one more after Stop. notes_in: 20 keyboard notes, 10 with every
note echoed back after 5 ms, a key released just after the sequenced note-off of the same note, then
10 echoed after 80 ms and Stop. hold_trig: 100 BPM for 6 s while the board's Life layer lights one tile
for a step and goes dark; its Hold note must end while the clock still runs. tempo_mod: no clock; on the board's own clock, with an LFO and CC1 both
on BPM, four fast CC1 sweeps, the last held at 64, then 6 s of the board's clock out. Every incoming message is stamped with perf_counter while the same loop
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
STEP_S, LOOP_S = 0.3, 2.4
# (seconds after Start, note, the Looper step it must land on); the last is played 1 s after Stop.
LOOPER_TAPS = [(0.010, 60, 0), (0.650, 61, 2), (1.450, 62, 5), (10.6, 63, 0)]
LOOPER_PLAYED_S = 4 * LOOP_S
HOLD_S = 0.06
NOTES_PLAIN = [(0.5 + 0.3 * i, 62 + i) for i in range(20)]
NOTES_ECHO_5 = [(7.0 + 0.3 * i, 62 + i) for i in range(10)]
NOTES_ECHO_80 = [(15.5 + 0.3 * i, 62 + i) for i in range(10)]
NOTES_PLAYED_S = 19.2
# Key 60 is held from 10.5 s; the sequenced note 60 sounds at 12.0 s for 150 ms.
HELD_KEY_S, SEQUENCED_OFF_S = 10.5, (12.08, 12.5)
HELD_KEY_RELEASED = []
# tempo_mod: CC1 sweeps 0-127-0 (254 messages in about 1 s) every 3 s on the board's own clock, the last
# one stopping at 64, which CC In maps to BPM 40 + 64/127 * 200.
TEMPO_SWEEP_STEP_S, TEMPO_SWEEPS, TEMPO_HELD_CC = 0.004, 4, 64
TEMPO_HELD_BPM = 40 + TEMPO_HELD_CC / 127 * 200
TEMPO_LISTEN_S = 2.0 + 3.0 * TEMPO_SWEEPS + 6.0


def port(ports, name):
    return next(i for i, label in enumerate(ports) if name in label and "Octessera" in label)


def drive(name, segments, sends=(), react=None, clock=True):
    """Plays Start, the clock segments and Stop, then listens for 3 s. sends holds (seconds after Start,
    message) pairs; react(seconds after Start, message) returns (delay, message) pairs to send back.
    Without clock it only sends and listens for the segments' length, leaving the board on its own clock."""
    ctypes.windll.winmm.timeBeginPeriod(1)
    ctypes.windll.kernel32.SetPriorityClass(ctypes.windll.kernel32.GetCurrentProcess(), 0x80)
    ctypes.windll.kernel32.SetThreadPriority(ctypes.windll.kernel32.GetCurrentThread(), 15)
    midi_in, midi_out = rtmidi.MidiIn(), rtmidi.MidiOut()
    midi_in.ignore_types(sysex=True, timing=False, active_sense=True)
    midi_in.open_port(port(midi_in.get_ports(), name))
    midi_out.open_port(port(midi_out.get_ports(), name))
    received, ticks, changes, late, sent_log = [], [], [], [], []
    origin = time.perf_counter() + 0.5
    pending = sorted((origin + offset, message) for offset, message in sends)

    def until(deadline):
        while (now := time.perf_counter()) < deadline:
            message = midi_in.get_message()
            if message:
                received.append((now, message[0]))
                if react:
                    pending.extend((now + delay, reply) for delay, reply in react(now - origin, message[0]))
                    pending.sort()
            while pending and pending[0][0] <= now:
                reply = pending.pop(0)[1]
                midi_out.send_message(reply)
                sent_log.append((time.perf_counter(), reply))

    until(origin)
    if not clock:
        until(origin + sum(seconds for _, seconds in segments))
        midi_in.close_port()
        midi_out.close_port()
        return {"start": origin, "stop": time.perf_counter(), "ticks": [], "changes": [], "received": received,
                "host_late": [], "sent": sent_log, "ccs": [(t, m[2]) for t, m in sent_log if m[0] == 0xB0]}
    midi_out.send_message([0xFA])
    sent = time.perf_counter()
    due = sent
    for bpm, seconds in segments:
        changes.append((due, bpm))
        for _ in range(round(seconds * bpm * 24 / 60)):
            until(due)
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
            "sent": sent_log, "ccs": [(t, message[2]) for t, message in sent_log if message[0] == 0xB0]}


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


def keys(taps, hold=0.15):
    return [(t, [0x90, note, 100]) for t, note in taps] + [(t + hold, [0x80, note, 0]) for t, note in taps]


def echo_notes(offset, message):
    """Echoes every note after 5 ms in its window and after 80 ms from 15.5 s on; releases the held key
    the moment the sequenced note-off of the same note arrives."""
    note = message[0] & 0xE0 == 0x80
    if (note and message[0] & 0xF0 == 0x80 and message[1] == 60 and not HELD_KEY_RELEASED
            and SEQUENCED_OFF_S[0] <= offset <= SEQUENCED_OFF_S[1]):
        HELD_KEY_RELEASED.append(offset)
        return [(0.002, [0x80, 60, 0])]
    if note and 7.0 <= offset < 10.3:
        return [(0.005, message)]
    if note and offset >= 15.5:
        return [(0.080, message)]
    return []


def analyze_looper(run):
    """Each tap sounds once live, lands on its step and replays once a pass, within half a step of its
    press time plus whole loops; the tap after Stop only sounds live."""
    problems, metrics = [], {}
    found = [n for n in notes(run["received"]) if n[0] == DRUM]
    for at, note, step in LOOPER_TAPS:
        pressed = run["start"] + at
        ons = [n[2] - run["start"] for n in found if n[1] == note]
        live = [t for t in ons if at <= t <= at + 0.1]
        replays = [t for t in ons if t > at + 0.1]
        expected = [at + k * LOOP_S for k in range(1, 5) if at + k * LOOP_S < LOOPER_PLAYED_S]
        if pressed > run["stop"]:
            expected = []
        misses = [e for e in expected if not any(abs(t - e) <= STEP_S / 2 for t in replays)]
        metrics[str(note)] = {"live": [round(t, 3) for t in live], "replays": [round(t, 3) for t in replays],
                              "replay_step_offsets_ms": [round(((t % LOOP_S) - step * STEP_S) * 1000, 1) for t in replays]}
        if len(live) != 1:
            problems.append(f"note {note} at {at} s sounded {len(live)} times live")
        if misses or len(replays) != len(expected):
            problems.append(f"note {note}: replays at {metrics[str(note)]['replays']}, expected one within half a step of each of {expected}")
    return problems, metrics


def analyze_notes_in(run):
    problems, metrics = [], {}
    found = [n for n in notes(run["received"]) if n[0] == DRUM]
    origin = run["start"]
    open_keys = [k for k in NOTES_PLAIN if not any(n[1] == k[1] and k[0] <= n[2] - origin <= k[0] + 0.1
                                                   and n[3] is not None and n[3] - origin <= k[0] + 0.15 + 1.0 for n in found)]
    metrics["plain_unreleased"] = len(open_keys)
    if open_keys:
        problems.append(f"{len(open_keys)} of 20 keys had no mirrored note-on and note-off within 1 s: {open_keys[:4]}")
    for label, taps in (("echo_5ms", NOTES_ECHO_5), ("echo_80ms", NOTES_ECHO_80)):
        counts = [sum(1 for n in found if n[1] == note and at <= n[2] - origin < at + 0.3) for at, note in taps]
        metrics[label] = counts
        if max(counts) > 2 or min(counts) < 1:
            problems.append(f"{label}: mirrored note-ons per key {counts}, not 1 or 2")
    after = [t - run["stop"] for t, message in run["received"] if t > run["stop"] + 1.0 and message[0] & 0xE0 == 0x80]
    metrics["notes_from_1s_after_stop"] = len(after)
    if after:
        problems.append(f"{len(after)} notes from 1 s after Stop on: the echo keeps feeding back")
    window = (SEQUENCED_OFF_S[0], SEQUENCED_OFF_S[1] + 0.01)
    released = [t for t, message in run["sent"] if message == [0x80, 60, 0] and window[0] <= t - origin <= window[1]]
    offs = [t for t, message in run["received"] if message[0] == 0x80 and message[1] == 60 and released
            and released[0] < t <= released[0] + 1.0]
    metrics["held_key"] = {"released_at": [round(t - origin, 3) for t in released], "mirrored_off_at": [round(t - origin, 3) for t in offs]}
    if len(released) != 1:
        problems.append("the sequenced note-off of the held key's note never came, so the key was not released")
    elif not offs:
        problems.append("the held key's mirrored note-off did not follow its release within 1 s")
    return problems, metrics


def tempo_sweeps():
    sends, at = [], 2.0
    for sweep in range(TEMPO_SWEEPS):
        down = range(126, TEMPO_HELD_CC - 1, -1) if sweep == TEMPO_SWEEPS - 1 else range(126, -1, -1)
        for value in [*range(0, 128), *down]:
            sends.append((at, [0xB0, 1, value]))
            at += TEMPO_SWEEP_STEP_S
        at += 2.0
    return sends


def analyze_tempo(run):
    """From one second after the last CC on, the board's clock out (LFO on BPM, 1/4) must swing and
    average the held BPM per pulse."""
    problems = []
    pulses = [t for t, message in run["received"] if message[0] == 0xF8 and t > run["ccs"][-1][0] + 1.0]
    bpms = [60 / 24 / (b - a) for a, b in zip(pulses, pulses[1:]) if b > a]
    beats = [statistics.fmean(bpms[i:i + 24]) for i in range(0, len(bpms) - 23, 24)]
    metrics = {"ccs_sent": len(run["ccs"]), "last_cc": run["ccs"][-1][1], "held_bpm": round(TEMPO_HELD_BPM, 2),
               "pulses_after_hold": len(pulses), "mean_bpm": round(statistics.fmean(bpms), 2) if bpms else None,
               "beat_bpm_span": [round(min(beats), 1), round(max(beats), 1)] if beats else None,
               "pulse_bpm_span": [round(min(bpms), 1), round(max(bpms), 1)] if bpms else None}
    if len(pulses) < 24 * 4:
        problems.append(f"only {len(pulses)} clock pulses came back after the last CC")
    elif abs(metrics["mean_bpm"] - TEMPO_HELD_BPM) > 3:
        problems.append(f"the clock averaged {metrics['mean_bpm']} BPM, not the held {TEMPO_HELD_BPM:.1f}")
    if bpms and max(bpms) - min(bpms) < 20:
        problems.append(f"the clock stayed within {metrics['pulse_bpm_span']} BPM: the LFO did not move it")
    return problems, metrics


def analyze_hold_trig(run):
    """Every mirrored Hold note must end while the clock still runs, at least 1 s before Stop."""
    found = [n for n in notes(run["received"]) if n[0] == DRUM]
    hanging = [n for n in found if n[3] is None or n[3] > run["stop"] - 1.0]
    metrics = {"notes": [[n[1], round(n[2] - run["start"], 3), None if n[3] is None else round(n[3] - run["start"], 3)]
                         for n in found], "stop_s": round(run["stop"] - run["start"], 3)}
    problems = [] if found else ["no Hold note sounded: the case did not set up its tile"]
    if hanging:
        problems.append(f"{len(hanging)} Hold notes still sounded 1 s before Stop, after the grid went dark: {metrics['notes'][:4]}")
    return problems, metrics


def analyze(run, phase):
    if phase == "tempo_mod":
        return analyze_tempo(run)
    if phase == "hold_trig":
        return analyze_hold_trig(run)
    if phase == "cc":
        return analyze_cc(run)
    if phase in ("looper", "notes_in"):
        return {"looper": analyze_looper, "notes_in": analyze_notes_in}[phase](run)
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
    segments = {"straight": [(100, 120), (140, 60)], "swing": [(100, 30)], "cc": [(100, 36)],
                "looper": [(100, LOOPER_PLAYED_S)], "notes_in": [(100, NOTES_PLAYED_S)],
                "hold_trig": [(100, 6)], "tempo_mod": [(None, TEMPO_LISTEN_S)]}[phase]
    sends = {"cc": [(t, [0xB0, 1, value]) for t, value in CC_STEPS],
             "looper": keys([(t, note) for t, note, _ in LOOPER_TAPS], HOLD_S),
             "notes_in": keys(NOTES_PLAIN + NOTES_ECHO_5 + NOTES_ECHO_80) + [(HELD_KEY_S, [0x90, 60, 100])],
             "tempo_mod": tempo_sweeps()}.get(phase, ())
    run = drive(name, segments, sends, echo_notes if phase == "notes_in" else None, clock=phase != "tempo_mod")
    problems, metrics = analyze(run, phase)
    with open(result, "w") as out:
        json.dump({"phase": phase, "problems": problems, "metrics": metrics, "run": run}, out)
    print(f"{'FAIL' if problems else 'PASS'} usb_midi {phase}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


if __name__ == "__main__":
    main()
