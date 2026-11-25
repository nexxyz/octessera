"""Repeated-pitch echo case against a board running the usb_echo_repeat scenario (run on the host).

usage: host_usb_midi_echo.py <Raspberry|Orange> <midi|synth> <result.json>
The board's L1 steps C4 on slot 1 every 1/16. midi: slot 1 is a MIDI Inst. on channel 1, so an echoed
note comes back as a channel 1 key and could be forwarded again; synth: slot 1 is a Synth mirrored by
Send MIDI all inst.
1. Start and 120 BPM clock for 4 s while every note from the board is echoed back after 3 ms, then
   Stop and 3 s more of echo: one C4 per step (32), none after Stop, and no echoed note-off cutting a
   note short: before Stop every C4 note-off pairs with a C4 note-on, and each lasts NOTE_MIN_MS or more.
2. (midi) Echo off, transport stopped: 10 key 60 presses 60 ms apart must all be forwarded (steady
   repeats under 40 ms are what the loop cap stops).
3. (midi) A key 60 press 150 ms after a sequenced C4 must not be forwarded (the 250 ms own-note rule).
4. (midi) Key 60 held for 1 s through a 300 ms echo: exactly one forwarded note-on and one note-off.
"""
import json
import sys

from host_usb_midi import DRUM, drive, notes

ECHO_S, STEPS = 0.003, 32
# The One-Shot Len is 150 ms; an echoed note-off passed on would end a note a few ms after it starts.
NOTE_MIN_MS = 100


def echo_all(offset, message):
    return [(ECHO_S, message)] if message[0] & 0xE0 == 0x80 else []


def c4_ons(run, start=float("-inf"), end=float("inf")):
    return [n for n in notes(run["received"]) if n[0] == DRUM and n[1] == 60 and start <= n[2] <= end]


def note_lengths(run):
    """C4 note lengths the board played before Stop (note-ons paired with note-offs in order), and the
    note-offs left over: an echoed note-off passed on would pair with a later note-on and end it early."""
    c4 = [(t, m) for t, m in run["received"] if m[0] & 0xEF == 0x80 and m[1] == 60 and t < run["stop"]]
    found = notes(c4)
    ons = len(found)
    offs = sum(1 for _, m in c4 if m[0] & 0xF0 == 0x80 or m[2] == 0)
    lengths = [(n[3] - n[2]) * 1000 for n in found if n[3] is not None]
    return (round(min(lengths), 1) if lengths else None), offs - ons


def echoed_steps(run):
    """One C4 per 1/16 step at 120 BPM while it plays, nothing from 0.2 s after Stop on, and no note
    ended early by an echoed note-off."""
    played = c4_ons(run, end=run["stop"] + 0.2)
    late = c4_ons(run, start=run["stop"] + 0.2)
    doubled = sum(1 for a, b in zip(played, played[1:]) if b[2] - a[2] < 0.06)
    shortest, extra_offs = note_lengths(run)
    problems = []
    if shortest is None or shortest < NOTE_MIN_MS or extra_offs > 0:
        problems.append(f"the shortest C4 lasted {shortest} ms and {extra_offs} note-offs had no note-on: an echoed "
                        f"note-off cut a note short (each must last {NOTE_MIN_MS} ms or more)")
    if abs(len(played) - STEPS) > 1 or doubled:
        problems.append(f"{len(played)} C4 note-ons while playing ({doubled} within 60 ms of the one before), "
                        f"not one per step ({STEPS})")
    if late:
        problems.append(f"{len(late)} C4 note-ons after Stop: the echo kept feeding back")
    return problems, {"played": len(played), "doubled": doubled, "after_stop": len(late), "shortest_note_ms": shortest,
                      "extra_note_offs": extra_offs}


def pads(port):
    """Echo off, stopped: 10 key 60 presses 60 ms apart, all forwarded."""
    sends = [(0.06 * i, [0x90, 60, 100]) for i in range(10)] + [(0.06 * i + 0.03, [0x80, 60, 0]) for i in range(10)]
    run = drive(port, [(None, 1.5)], sends, clock=False)
    forwarded = len(c4_ons(run))
    return ([] if forwarded == 10 else [f"{forwarded} of 10 quick key 60 presses forwarded"]), {"pads_forwarded": forwarded}


def own_note(port):
    """A key 60 press 150 ms after a sequenced C4 is taken for that note's echo and not forwarded."""
    pressed = []

    def press_after_c4(offset, message):
        if pressed or message[0] & 0xF0 != 0x90 or message[1] != 60 or message[2] == 0:
            return []
        pressed.append(offset)
        return [(0.15, [0x90, 60, 100]), (0.2, [0x80, 60, 0])]

    run = drive(port, [(120, 0.1)], react=press_after_c4)
    key = next((t for t, message in run["sent"] if message == [0x90, 60, 100]), None)
    forwarded = c4_ons(run, start=key) if key else []
    problems = [] if key else ["no sequenced C4 came to press after"]
    if forwarded:
        problems.append(f"the key press 150 ms after a sequenced C4 was forwarded ({len(forwarded)} note-ons)")
    return problems, {"own_note_forwarded": len(forwarded)}


def held_key(port):
    """A key held through a slow echo is forwarded once: its echoed note-on comes back while the key
    is still down and is taken for the echo."""
    run = drive(port, [(None, 2.5)], [(0.0, [0x90, 60, 100]), (1.0, [0x80, 60, 0])],
                react=lambda offset, message: [(0.3, message)] if message[0] & 0xE0 == 0x80 else [], clock=False)
    channel_one = [m for _, m in run["received"] if m[0] & 0xEF == 0x80 and m[1] == 60]
    ons = sum(1 for m in channel_one if m[0] == 0x90 and m[2] > 0)
    offs = len(channel_one) - ons
    problems = [] if (ons, offs) == (1, 1) else [f"a key held through a 300 ms echo was forwarded as {ons} note-ons "
                                                 f"and {offs} note-offs, not 1 and 1"]
    return problems, {"held_key_ons": ons, "held_key_offs": offs}


def main():
    port, kind, result = sys.argv[1:4]
    run = drive(port, [(120, 4)], react=echo_all)
    problems, metrics = echoed_steps(run)
    if kind == "midi":
        for part in (pads, own_note, held_key):
            more, facts = part(port)
            problems += more
            metrics.update(facts)
    with open(result, "w") as out:
        json.dump({"phase": f"echo_{kind}", "problems": problems, "metrics": metrics, "run": run}, out)
    print(f"{'FAIL' if problems else 'PASS'} usb_echo_repeat {kind}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


if __name__ == "__main__":
    main()
