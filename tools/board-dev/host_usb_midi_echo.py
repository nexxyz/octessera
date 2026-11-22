"""Repeated-pitch echo case against a board running the usb_echo_repeat scenario (run on the host).

usage: host_usb_midi_echo.py <Raspberry|Orange> <midi|synth> <result.json>
The board's L1 steps C4 on slot 1 every 1/16. midi: slot 1 is a MIDI Inst. on channel 1, so an echoed
note comes back as a channel 1 key and could be forwarded again; synth: slot 1 is a Synth mirrored by
Send MIDI all inst.
1. Start and 120 BPM clock for 4 s while every note from the board is echoed back after 3 ms, then
   Stop and 3 s more of echo: one C4 per step (32) and none after Stop.
2. (midi) Echo off, transport stopped: 10 key 60 presses 40 ms apart must all be forwarded.
3. (midi) A key 60 press 300 ms after a sequenced C4 must not be forwarded (the 1 s own-note rule).
"""
import json
import sys

from host_usb_midi import DRUM, drive, notes

ECHO_S, STEPS = 0.003, 32


def echo_all(offset, message):
    return [(ECHO_S, message)] if message[0] & 0xE0 == 0x80 else []


def c4_ons(run, start=float("-inf"), end=float("inf")):
    return [n for n in notes(run["received"]) if n[0] == DRUM and n[1] == 60 and start <= n[2] <= end]


def echoed_steps(run):
    """One C4 per 1/16 step at 120 BPM while it plays, and nothing from 0.2 s after Stop on."""
    played = c4_ons(run, end=run["stop"] + 0.2)
    late = c4_ons(run, start=run["stop"] + 0.2)
    doubled = sum(1 for a, b in zip(played, played[1:]) if b[2] - a[2] < 0.06)
    problems = []
    if abs(len(played) - STEPS) > 1 or doubled:
        problems.append(f"{len(played)} C4 note-ons while playing ({doubled} within 60 ms of the one before), "
                        f"not one per step ({STEPS})")
    if late:
        problems.append(f"{len(late)} C4 note-ons after Stop: the echo kept feeding back")
    return problems, {"played": len(played), "doubled": doubled, "after_stop": len(late)}


def pads(port):
    """Echo off, stopped: 10 key 60 presses 40 ms apart, all forwarded."""
    sends = [(0.04 * i, [0x90, 60, 100]) for i in range(10)] + [(0.04 * i + 0.02, [0x80, 60, 0]) for i in range(10)]
    run = drive(port, [(None, 1.5)], sends, clock=False)
    forwarded = len(c4_ons(run))
    return ([] if forwarded == 10 else [f"{forwarded} of 10 quick key 60 presses forwarded"]), {"pads_forwarded": forwarded}


def own_note(port):
    """A key 60 press 300 ms after a sequenced C4 is taken for that note's echo and not forwarded."""
    pressed = []

    def press_after_c4(offset, message):
        if pressed or message[0] & 0xF0 != 0x90 or message[1] != 60 or message[2] == 0:
            return []
        pressed.append(offset)
        return [(0.3, [0x90, 60, 100]), (0.35, [0x80, 60, 0])]

    run = drive(port, [(120, 0.1)], react=press_after_c4)
    key = next((t for t, message in run["sent"] if message == [0x90, 60, 100]), None)
    forwarded = c4_ons(run, start=key) if key else []
    problems = [] if key else ["no sequenced C4 came to press after"]
    if forwarded:
        problems.append(f"the key press 300 ms after a sequenced C4 was forwarded ({len(forwarded)} note-ons)")
    return problems, {"own_note_forwarded": len(forwarded)}


def main():
    port, kind, result = sys.argv[1:4]
    run = drive(port, [(120, 4)], react=echo_all)
    problems, metrics = echoed_steps(run)
    if kind == "midi":
        for part in (pads, own_note):
            more, facts = part(port)
            problems += more
            metrics.update(facts)
    with open(result, "w") as out:
        json.dump({"phase": f"echo_{kind}", "problems": problems, "metrics": metrics}, out)
    print(f"{'FAIL' if problems else 'PASS'} usb_echo_repeat {kind}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


if __name__ == "__main__":
    main()
