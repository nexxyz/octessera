"""Two layers on one pitch against a board running the usb_two_layers scenario (run on the host).

usage: host_usb_midi_layers.py <Raspberry|Orange> <l1|l2> <result.json>
The board's L1 scans row 0 columns 0-3 and L2 columns 2-5 at 1/8, every tile on note 60 of a Hold Synth
mirrored by Send MIDI all inst.; l1 and l2 say which layer is selected. Start and 100 BPM clock for
12 s (five 8-step passes of 2.4 s), then Stop. Each pass must leave note 60 released by its last step,
no note may sound after Stop, and the pass pattern (each note-on and note-off by its step) is written
out so the board check can require the same pattern with either layer selected.
"""
import json
import sys

from host_usb_midi import DRUM, drive

STEP_S, PASS_STEPS, PASSES = 0.3, 8, 5


def key_events(run):
    return [(t, "on" if m[0] & 0xF0 == 0x90 and m[2] > 0 else "off") for t, m in run["received"]
            if (m[0] & 0xEF) == 0x80 | DRUM and m[1] == 60]


def analyze(run):
    events = key_events(run)
    first = next((t for t, kind in events if kind == "on"), None)
    if first is None:
        return ["no note 60 came back: the layers did not play"], {}
    problems, patterns, held_at_end = [], [], []
    for index in range(1, PASSES - 1):
        start = first + index * PASS_STEPS * STEP_S - STEP_S / 2
        window = [(t, kind) for t, kind in events if start <= t < start + PASS_STEPS * STEP_S]
        patterns.append([[round((t - start) / STEP_S - 0.5), kind] for t, kind in window])
        last_step = start + (PASS_STEPS - 1) * STEP_S + STEP_S / 2
        held = sum(1 if kind == "on" else -1 for t, kind in events if t <= last_step)
        held_at_end.append(held)
    if any(held > 0 for held in held_at_end):
        problems.append(f"note 60 still held at the last step of passes 2-4 (on minus off {held_at_end})")
    if any(pattern != patterns[0] for pattern in patterns):
        problems.append(f"the passes differ: {patterns}")
    after = [t for t, kind in events if kind == "on" and t > run["stop"] + 0.05]
    balance = sum(1 if kind == "on" else -1 for _, kind in events)
    if after or balance > 0:
        problems.append(f"after Stop: {len(after)} note-ons, {balance} notes never ended")
    return problems, {"pattern": patterns[0], "held_at_last_step": held_at_end, "notes_after_stop": len(after),
                      "unended": balance}


def main():
    port, selected, result = sys.argv[1:4]
    run = drive(port, [(100, PASSES * PASS_STEPS * STEP_S)])
    problems, metrics = analyze(run)
    with open(result, "w") as out:
        json.dump({"phase": f"two_layers_{selected}", "problems": problems, "metrics": metrics, "run": run}, out)
    print(f"{'FAIL' if problems else 'PASS'} usb_two_layers {selected}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


if __name__ == "__main__":
    main()
