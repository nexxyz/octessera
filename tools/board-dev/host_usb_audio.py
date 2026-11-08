"""Captures a board's USB Audio output on the host and checks level, dropouts and clipping (run on the host).

usage: host_usb_audio.py <seconds> <result-stem> [baseline.json | fx]
Waits for the board's capture device, records <stem>.wav and writes <stem>.json. It records through
WASAPI in exclusive mode: shared-mode capture (ffmpeg dshow) runs the Windows microphone effects,
which filtered the clipped tops and slowly turned the level down. Windows does not name the board in
its capture device ("Octessera Audio", "Octessera Line In"), so only one board may have USB Audio on.
With a baseline file the take is a level run: RMS within 3 dB of the baseline (recorded by the first
run), no exact-zero run of 64 frames or more while sounding, and no full-scale sample. Without one
it only reports the full-scale runs a clipping patch should show. With fx the take holds a tape stop
on master: the level must fall at least 20 dB below the take's opening level for 3 s or more, then
come back within 3 dB of it. Needs numpy and sounddevice.
"""
import json
import sys
import time
import wave
from pathlib import Path

import numpy as np
import sounddevice

FULL_SCALE = 32767
FX_DIP_DB = 20
FX_DIP_S = 3
# The first seconds after the host opens the stream are judged apart: there the board's USB stream
# can still be settling, and the take starts with the host's own leading silence.
STARTUP_S = 2


def device(timeout=90.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        sounddevice._terminate()
        sounddevice._initialize()
        wasapi = next(i for i, api in enumerate(sounddevice.query_hostapis()) if "WASAPI" in api["name"])
        found = [i for i, d in enumerate(sounddevice.query_devices())
                 if d["hostapi"] == wasapi and d["max_input_channels"] >= 2 and "Octessera" in d["name"]]
        if len(found) > 1:
            sys.exit("more than one Octessera capture device")
        if found:
            return found[0]
        time.sleep(3)
    sys.exit("no Octessera capture device appeared")


def runs(mask):
    """(start, length) of every run of True in a boolean array."""
    edges = np.flatnonzero(np.diff(np.concatenate(([0], mask.astype(np.int8), [0]))))
    return list(zip(edges[::2], edges[1::2] - edges[::2]))


def analyze_fx(level, rate):
    """A held tape stop: a long dip well below the opening level, and the level back after it."""
    block = rate // 20
    blocks = 20 * np.log10(np.sqrt(np.mean(level[: len(level) // block * block].reshape(-1, block, level.shape[1]) ** 2,
                                           axis=(1, 2))) + 1e-12)
    before = float(np.median(blocks[: len(blocks) // 4]))
    dips = runs(blocks < before - FX_DIP_DB)
    start, length = (int(value) for value in max(dips, key=lambda run: run[1], default=(0, 0)))
    after = blocks[start + length + 20:]
    back = float(np.median(after)) if len(after) else float("-inf")
    metrics = {"opening_rms_dbfs": round(before, 2), "dip_s": round(length / 20, 2),
               "dip_start_s": round(start / 20, 2), "after_rms_dbfs": round(back, 2)}
    problems = []
    if before < -50:
        problems.append("the take is silent before the hold")
    if length < FX_DIP_S * 20:
        problems.append(f"the level dipped {FX_DIP_DB} dB below the opening for only {length / 20:.2f} s")
    if abs(back - before) > 3:
        problems.append(f"after the release the level is {back:.2f} dBFS, not within 3 dB of {before:.2f}")
    return problems, metrics


def analyze(path, baseline):
    with wave.open(str(path)) as take:
        rate = take.getframerate()
        frames = np.frombuffer(take.readframes(take.getnframes()), dtype="<i2").reshape(-1, take.getnchannels())
    level = frames.astype(np.float64) / 32768
    block = rate // 20
    blocks = np.sqrt(np.mean(level[: len(level) // block * block].reshape(-1, block, level.shape[1]) ** 2, axis=(1, 2)))
    sounding = np.repeat(blocks > 10 ** (-50 / 20), block)
    silent = runs(np.all(frames[: len(sounding)] == 0, axis=1))
    gaps = [(int(s), int(n)) for s, n in silent if n >= 64 and sounding[min(s + n, len(sounding) - 1)]
            and (s == 0 or sounding[s - 1])]
    dropouts = [gap for gap in gaps if gap[0] >= STARTUP_S * rate]
    frames, level = frames[STARTUP_S * rate:], level[STARTUP_S * rate:]
    if baseline == "fx":
        return analyze_fx(level, rate)
    rms = float(20 * np.log10(np.sqrt(np.mean(level ** 2)) + 1e-12))
    loud = np.abs(frames).max(axis=1) >= FULL_SCALE
    flat = [int(n) for _, n in runs(loud) if n >= 3]
    metrics = {"seconds": round(len(frames) / rate, 2), "rms_dbfs": round(rms, 2), "peak": int(np.abs(frames).max()),
               "full_scale_samples": int(loud.sum()), "flat_top_runs": len(flat), "longest_flat_top": max(flat, default=0),
               "dropouts": dropouts[:20], "dropout_count": len(dropouts),
               "startup_gaps": [gap for gap in gaps if gap[0] < STARTUP_S * rate]}
    problems = []
    if baseline is None:
        if not flat:
            problems.append("no flat-topped full-scale runs in the clipping take")
        return problems, metrics
    if metrics["seconds"] < 80:
        problems.append(f"capture is only {metrics['seconds']} s")
    if baseline.exists():
        reference = json.loads(baseline.read_text())["rms_dbfs"]
        metrics["baseline_rms_dbfs"] = reference
        if abs(rms - reference) > 3:
            problems.append(f"RMS {rms:.2f} dBFS is more than 3 dB from the baseline {reference:.2f}")
    else:
        baseline.parent.mkdir(parents=True, exist_ok=True)
        baseline.write_text(json.dumps({"rms_dbfs": round(rms, 2), "recorded": time.strftime("%Y-%m-%d %H:%M")}))
        metrics["baseline_rms_dbfs"] = "recorded now"
    if dropouts:
        problems.append(f"{len(dropouts)} exact-zero runs of 64+ frames while sounding")
    if metrics["full_scale_samples"]:
        problems.append(f"{metrics['full_scale_samples']} full-scale samples")
    if rms < -50:
        problems.append("the take is silent")
    return problems, metrics


def main():
    seconds, stem = float(sys.argv[1]), Path(sys.argv[2])
    baseline = None if len(sys.argv) < 4 else "fx" if sys.argv[3] == "fx" else Path(sys.argv[3])
    wav = stem.with_suffix(".wav")
    frames = sounddevice.rec(int((seconds + STARTUP_S) * 44100), samplerate=44100, channels=2, dtype="int16", device=device(),
                             extra_settings=sounddevice.WasapiSettings(exclusive=True))
    sounddevice.wait()
    with wave.open(str(wav), "wb") as take:
        take.setnchannels(2)
        take.setsampwidth(2)
        take.setframerate(44100)
        take.writeframes(frames.tobytes())
    problems, metrics = analyze(wav, baseline)
    stem.with_suffix(".json").write_text(json.dumps({"problems": problems, "metrics": metrics}))
    print(f"{'FAIL' if problems else 'PASS'} {stem.name}: {json.dumps(metrics)}")
    for problem in problems:
        print(f"  - {problem}")


main()
