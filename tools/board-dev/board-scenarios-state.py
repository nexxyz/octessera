"""Prints one line of facts about a cloned board store (run as root on the board).

usage: board-scenarios-state.py presets <store-dir> <label> <diff-log>
       board-scenarios-state.py store <store-dir> <label>
presets: the saved patch's BPM and L1 One-Shot Len, the presets whose every runtimeConfig value it holds,
         and a hash per preset file; appends where the saved patch differs from preset A to diff-log.
store:   the layer behaviors, BPM, the System values the store scenarios change, new set-aside
         copies, the inodes of system.json and its set-aside copies, empty or unparsable .json files
         and leftover temporary files.
"""
import hashlib
import json
import sys
from pathlib import Path

SYSTEM_KEYS = ("gridBrightness", "buttonBrightness", "autoSaveDefault", "dimTimerSeconds", "screenSleepSeconds")


def music(path):
    """runtimeConfig without the delay times a note-mode delay derives from BPM: the runtime
    recomputes those on load (the factory patch stores 273 ms for a 1/8 at 120 BPM)."""
    config = json.loads(path.read_text())["runtimeConfig"]
    for bus in config.get("mixer", {}).get("buses", []):
        for slot in bus.values():
            if isinstance(slot, dict) and slot.get("params", {}).get("timeMode") == "note":
                slot["params"].pop("timeMs", None)
    return config


def differ(a, b, path=""):
    if isinstance(a, dict) and isinstance(b, dict):
        return [d for k in sorted(set(a) | set(b)) for d in differ(a.get(k), b.get(k), f"{path}.{k}")]
    if isinstance(a, list) and isinstance(b, list) and len(a) == len(b):
        return [d for i, (x, y) in enumerate(zip(a, b)) for d in differ(x, y, f"{path}[{i}]")]
    return [] if a == b else [f"{path}={json.dumps(a)}->{json.dumps(b)}"]


def presets(store, label, diff_log):
    files = sorted((store / "patches").glob("*.json"))
    hashes = " ".join(f"{p.stem}={hashlib.sha256(p.read_bytes()).hexdigest()[:16]}" for p in files)
    saved = music(store / "default.patch.json")
    loaded = [p.stem for p in files if all(saved.get(k) == v for k, v in music(p).items())]
    if (store / "patches" / "A.json").exists():
        with open(diff_log, "a") as out:
            out.write(f"{label} {' '.join(differ(music(store / 'patches' / 'A.json'), saved)[:20])}\n")
    return [label, f"bpm={saved['transport']['bpm']}", f"note={saved['layers'][0]['link']['oneShotLengthMs']}",
            f"matches={','.join(loaded) or '-'}", hashes]


def store_facts(store, label):
    facts = [label]
    try:
        patch = music(store / "default.patch.json")
        facts += [f"behaviors={','.join(sorted({layer['build']['behaviorId'] for layer in patch['layers']}))}",
                  f"bpm={patch['transport']['bpm']}"]
    except (OSError, ValueError, KeyError) as error:
        facts.append(f"patch_error={type(error).__name__}")
    try:
        system = json.loads((store / "system.json").read_text())["runtimeConfig"]
        facts += [f"{key}={json.dumps(system[key], separators=(',', ':'))}" for key in SYSTEM_KEYS]
        facts.append(f"notesIn={json.dumps(system['midi'].get('notesIn'))}")
        facts.append(f"system_inode={(store / 'system.json').stat().st_ino}")
    except (OSError, ValueError, KeyError) as error:
        facts.append(f"system_error={type(error).__name__}")
    aside = sorted(store.glob("system.json.pre-reset-*"))
    facts.append(f"pre_reset={len(aside)}")
    facts.append(f"pre_reset_inodes={','.join(str(p.stat().st_ino) for p in aside) or '-'}")
    names = sorted(p.name for p in store.rglob("*") if p.is_file() and p.name != ".filler")
    broken = []
    for p in store.rglob("*.json"):
        try:
            json.loads(p.read_text()) if p.stat().st_size else broken.append(f"{p.name}:empty")
        except ValueError:
            broken.append(f"{p.name}:unparsable")
    facts.append(f"broken={','.join(sorted(broken)) or '-'}")
    facts.append(f"temporary={','.join(n for n in names if n.endswith('.tmp') or n.startswith('.')) or '-'}")
    facts.append(f"presets={len(list((store / 'patches').glob('*.json')))}")
    return facts


def main():
    mode, store, label = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
    facts = presets(store, label, sys.argv[4]) if mode == "presets" else store_facts(store, label)
    print(" ".join(facts))


if __name__ == "__main__":
    main()
