"""Checks for the scenarios that reset, damage or fill the cloned store (board-scenarios-stores.sh);
each reads the key=value lines in stores.log."""
import importlib.util
import json
from pathlib import Path

from board_scenarios_evidence import fields, journal, marks, window_errors

HERE = Path(__file__).resolve().parent
FACTORY_CONFIG = HERE.parents[1] / "config" / "generated" / "pi" / "default.json"
_state = importlib.util.spec_from_file_location("board_scenarios_state", HERE / "board-scenarios-state.py")
state = importlib.util.module_from_spec(_state)
_state.loader.exec_module(state)


def store_lines(out):
    return (out / "stores.log").read_text().splitlines()


def store_rows(out):
    """The labelled lines: a label, then key=value pairs."""
    return dict(fields(line) for line in store_lines(out) if "=" not in line.split()[0])


def store_facts(out):
    """The key=value pairs of every line, labelled or not; a later line wins."""
    return dict(pair.split("=", 1) for line in store_lines(out) for pair in line.split() if "=" in pair)


def set_values_differ(factory, saved, path=""):
    """Where the two disagree, leaving out a null on either side: the runtime fills the factory's
    null settings of instrument types a slot does not use, and the patch leaves System-owned keys
    (sound.optimizeFor) to system.json."""
    if factory is None or saved is None:
        return []
    if isinstance(factory, dict) and isinstance(saved, dict):
        return [d for k in sorted(set(factory) | set(saved))
                for d in set_values_differ(factory.get(k), saved.get(k), f"{path}.{k}")]
    if isinstance(factory, list) and isinstance(saved, list) and len(factory) == len(saved):
        return [d for i, (f, s) in enumerate(zip(factory, saved)) for d in set_values_differ(f, s, f"{path}[{i}]")]
    return [] if factory == saved else [f"{path}={json.dumps(factory)}->{json.dumps(saved)}"]


def factory_patch_problems(saved_path):
    """The saved patch must hold the patch half of config/generated/pi/default.json."""
    saved = state.music(saved_path)
    factory = state.music(FACTORY_CONFIG)
    unknown = sorted(set(saved) - set(factory))
    differs = set_values_differ({k: factory[k] for k in saved if k in factory}, saved)
    return ([f"keys the factory config lacks: {unknown}"] if unknown else []) + \
        ([f"differs from the factory config: {differs[:5]}"] if differs else [])


def resets(board, out):
    rows, facts = store_rows(out), store_facts(out)
    factory = json.loads(FACTORY_CONFIG.read_text())["runtimeConfig"]
    expected = {key: json.dumps(factory[key], separators=(",", ":")) for key in state.SYSTEM_KEYS}
    initial, before, edited, restarted = rows["initial"], rows["factory"], rows["system_edit"], rows["restarted"]
    new_aside = set(restarted["pre_reset_inodes"].split(",")) - set(initial["pre_reset_inodes"].split(","))
    problems = []
    if rows["empty"].get("behaviors") != "none":
        problems.append(f"Empty Patch saved layers {rows['empty'].get('behaviors')}, not all none")
    problems += [f"Factory Patch: {p}" for p in factory_patch_problems(out / "resets-factory-patch.json")]
    if [int(edited[k]) - int(before[k]) for k in ("gridBrightness", "buttonBrightness")] != [10, 10]:
        problems.append("the two System edits were not saved")
    if facts["pid_before_reset"] != facts["pid_after_reset"]:
        problems.append("the app restarted during Factory System although the restart was declined")
    changed = {key: restarted.get(key) for key in state.SYSTEM_KEYS if restarted.get(key) != expected[key]}
    if changed:
        problems.append(f"after Factory System and a restart these differ from factory {expected}: {changed}")
    if int(restarted["pre_reset"]) - int(initial["pre_reset"]) != 1:
        problems.append(f"{int(restarted['pre_reset']) - int(initial['pre_reset'])} new set-aside copies, not 1")
    if edited["system_inode"] in new_aside:
        problems.append("system.json was renamed aside instead of copied")
    if facts["missing"] != "0":
        problems.append(f"system.json was missing in {facts['missing']} of the 20 ms polls")
    problems += window_errors(out, "resets")
    return problems, (f"empty layers {rows['empty'].get('behaviors')}, after Factory System and restart "
                      f"{[restarted.get(k) for k in state.SYSTEM_KEYS]}, {len(new_aside)} new copy set aside, "
                      f"pid {facts['pid_before_reset']} -> {facts['pid_after_reset']}, {facts['missing']} missing polls")


def bad_stores(board, out):
    """The restore screen's choices take only the hardware encoders, so the recovery pick is not
    driven; the announcement, the restart count and the kept files are."""
    rows = store_rows(out)
    broken = ("unknown_key", "unknown_runtime_key", "truncated_patch")
    problems = []
    if rows["old_system_saved"].get("notesIn") != "true":
        problems.append(f"a System file without notesIn saved notesIn {rows['old_system_saved'].get('notesIn')}")
    for name in broken:
        row = rows[name]
        if (row["announced"], row["restarts"], row["active"], row["files_intact"]) != ("1", "0", "active", "yes"):
            problems.append(f"{name}: announced {row['announced']}x, {row['restarts']} restarts, {row['active']}, "
                            f"files intact {row['files_intact']}")
    shown = "; ".join(f"{name} {dict((k, v) for k, v in rows[name].items() if k != 'message')}" for name in broken)
    return problems, f"{shown}; old System file saved notesIn {rows['old_system_saved'].get('notesIn')}"


def full_card(board, out):
    rows, facts = store_rows(out), store_facts(out)
    found = marks(out)
    rows_journal = journal(out / "journal.log")
    storage = [text for t, _, text in rows_journal if found["full_card_begin"] <= t <= found["full_card_end"]
               and ("No space left" in text or "domain=storage" in text)]
    full_edit_errors = [text for t, _, text in rows_journal
                        if found["full_edits_begin"] <= t <= found["full_freed"] and "No space left" in text]
    full, edits, freed, saved = rows["full_save_as"], rows["full_edits"], rows["freed_autosave"], rows["freed_save_as"]
    problems = []
    if rows["filled"]["copied"] != "yes" or int(rows["filled"]["free_kb"]) > 64:
        problems.append(f"the card image: copied {rows['filled']['copied']}, {rows['filled']['free_kb']} kB free")
    for step in ("full_save_as", "full_edits", "freed_autosave", "freed_save_as"):
        if rows[step].get("broken") != "-":
            problems.append(f"after {step}: broken files {rows[step].get('broken')}")
    if edits["bpm"] != full["bpm"] or int(edits["gridBrightness"]) != int(full["gridBrightness"]) + 10:
        problems.append(f"while full: BPM {full['bpm']} -> {edits['bpm']} (must stay), Grid Bright "
                        f"{full['gridBrightness']} -> {edits['gridBrightness']} (the small System file must land)")
    if not storage or not full_edit_errors:
        problems.append(f"the journal named the full card {len(storage)}x, {len(full_edit_errors)}x for the BPM edit")
    if float(freed["bpm"]) != float(full["bpm"]) + 15:
        problems.append(f"after freeing space the autosave kept BPM {freed['bpm']}")
    if facts["same_pid"] != "yes" or int(saved["presets"]) != int(edits["presets"]) + 1:
        problems.append(f"after freeing space: same process {facts['same_pid']}, presets {edits['presets']} -> {saved['presets']}")
    return problems, (f"{rows['filled']['free_kb']} kB free, {len(storage)} storage lines (first: {storage[:1]}), "
                      f"temporary files {saved.get('temporary')}, BPM {full['bpm']} while full, {freed['bpm']} after "
                      f"freeing, presets {edits['presets']} -> {saved['presets']}, same process {facts['same_pid']}")
