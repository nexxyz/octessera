"""Checks for the scenarios that reset, damage or fill the cloned store (board-scenarios-stores.sh);
each reads the key=value lines in stores.log."""
import json
from pathlib import Path

from board_scenarios_evidence import fields, journal, marks, window_errors

FACTORY_CONFIG = Path(__file__).resolve().parents[2] / "config" / "generated" / "pi" / "default.json"
RESET_KEYS = ("gridBrightness", "buttonBrightness", "autoSaveDefault", "dimTimerSeconds", "screenSleepSeconds")


def store_lines(out):
    return (out / "stores.log").read_text().splitlines()


def store_rows(out):
    """The labelled lines: a label, then key=value pairs."""
    return dict(fields(line) for line in store_lines(out) if "=" not in line.split()[0])


def store_facts(out):
    """The key=value pairs of every line, labelled or not; a later line wins."""
    return dict(pair.split("=", 1) for line in store_lines(out) for pair in line.split() if "=" in pair)


def resets(board, out):
    rows = store_rows(out)
    factory = json.loads(FACTORY_CONFIG.read_text())["runtimeConfig"]
    expected = {key: json.dumps(factory[key], separators=(",", ":")) for key in RESET_KEYS}
    before, edited, restarted = rows["factory"], rows["system_edit"], rows["restarted"]
    missing = store_facts(out)["missing"]
    problems = []
    if rows["empty"].get("behaviors") != "none":
        problems.append(f"Empty Patch saved layers {rows['empty'].get('behaviors')}, not all none")
    if rows["factory"].get("factory_patch") != "yes":
        problems.append("Factory Patch did not save the factory patch")
    if [int(edited[k]) - int(before[k]) for k in ("gridBrightness", "buttonBrightness")] != [10, 10]:
        problems.append("the two System edits were not saved")
    changed = {key: restarted.get(key) for key in RESET_KEYS if restarted.get(key) != expected[key]}
    if changed:
        problems.append(f"after Factory System and a restart these differ from factory {expected}: {changed}")
    if restarted.get("pre_reset") != "1":
        problems.append(f"{restarted.get('pre_reset')} set-aside copies of system.json, not 1")
    if missing != "0":
        problems.append(f"system.json was missing in {missing} of the 20 ms polls")
    problems += window_errors(out, "resets")
    return problems, (f"empty layers {rows['empty'].get('behaviors')}, factory patch {rows['factory'].get('factory_patch')}, "
                      f"after Factory System and restart {[restarted.get(k) for k in RESET_KEYS]}, "
                      f"{restarted.get('pre_reset')} set aside, {missing} missing polls")


def bad_stores(board, out):
    """The restore screen's choices take only the hardware encoders, so the recovery pick is not
    driven; the announcement, the restart count and the kept files are."""
    rows = store_rows(out)
    problems = []
    if rows["old_system_saved"].get("notesIn") != "true":
        problems.append(f"a System file without notesIn saved notesIn {rows['old_system_saved'].get('notesIn')}")
    for name in ("unknown_key", "truncated_patch"):
        row = rows[name]
        if (row["announced"], row["restarts"], row["active"], row["files_intact"]) != ("1", "0", "active", "yes"):
            problems.append(f"{name}: announced {row['announced']}x, {row['restarts']} restarts, {row['active']}, "
                            f"files intact {row['files_intact']}")
    shown = "; ".join(f"{name} {dict((k, v) for k, v in rows[name].items() if k != 'message')}"
                      for name in ("unknown_key", "truncated_patch"))
    return problems, f"{shown}; old System file saved notesIn {rows['old_system_saved'].get('notesIn')}"


def full_card(board, out):
    rows = store_rows(out)
    facts = store_facts(out)
    found = marks(out)
    storage = [text for t, _, text in journal(out / "journal.log")
               if found["full_card_begin"] <= t <= found["full_card_end"]
               and ("No space left" in text or "domain=storage" in text)]
    problems = []
    if int(facts["free_kb"]) > 64:
        problems.append(f"the card image kept {facts['free_kb']} kB free")
    for step in ("full_save_as", "full_edits", "freed_autosave", "freed_save_as"):
        if rows[step].get("broken") != "-":
            problems.append(f"after {step}: broken files {rows[step].get('broken')}")
    if not storage:
        problems.append("the journal never named the full card")
    if int(rows["freed_autosave"]["bpm"]) != int(rows["full_save_as"]["bpm"]) + 15:
        problems.append(f"after freeing space the autosave kept BPM {rows['freed_autosave']['bpm']}")
    if facts["same_pid"] != "yes" or facts["presets"] == "0":
        problems.append(f"after freeing space: same process {facts['same_pid']}, {facts['presets']} presets saved")
    return problems, (f"{facts['free_kb']} kB free, {len(storage)} storage lines (first: {storage[:1]}), "
                      f"temporary files {rows['freed_save_as'].get('temporary')}, BPM after freeing "
                      f"{rows['freed_autosave']['bpm']}, {facts['presets']} presets, same process {facts['same_pid']}")
