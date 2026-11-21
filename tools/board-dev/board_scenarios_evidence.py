"""Reads the evidence board-scenarios.sh collects: the service journal, the payload's marks and the
key=value lines its scenarios log. Shared by the scenario checks."""
import re
from pathlib import Path

ERROR = re.compile(r"(?i)\b(fail\w*|error|panicked|unavailable)\b")
BENIGN = ("HDMI framebuffer waiting",)
JOURNAL = re.compile(r"^\[\s*([\d.]+)\] \S+ ([^\[:]+)(?:\[\d+\])?: (.*)$")


def journal(path):
    rows = []
    for line in Path(path).read_text(errors="replace").splitlines():
        match = JOURNAL.match(line)
        if match:
            rows.append((float(match.group(1)), match.group(2), match.group(3)))
    return rows


def errors(rows, start=0.0, end=float("inf")):
    return [f"{t:.1f} {ident}: {text}" for t, ident, text in rows
            if start <= t <= end and ident.startswith(("octessera", "systemd"))
            and ("octessera" in ident or "octessera" in text)
            and ERROR.search(text) and not any(b in text for b in BENIGN)]


def marks(out):
    found = {}
    for line in (out / "marks.log").read_text().splitlines():
        t, name = line.split(" ", 1)
        found.setdefault(name.split(" ")[0], float(t))
    return found


def window_errors(out, name):
    found = marks(out)
    return errors(journal(out / "journal.log"), found[f"{name}_begin"], found[f"{name}_end"])


def first(rows, needle, ident=None):
    return next((t for t, who, text in rows if needle in text and (ident is None or who == ident)), None)

def window_counts(out, start, end):
    found = marks(out)
    rows = [r for r in journal(out / "journal.log") if found[start] <= r[0] <= found[end]]
    underruns = sum("underrun occurred" in text for _, _, text in rows)
    late = sum(int(m.group(1)) for _, _, text in rows for m in [re.search(r"late timed events: (\d+)", text)] if m)
    worst = max((int(m.group(1)) for _, _, text in rows for m in [re.search(r"runtime_late=n=\d+ avg=\d+us max=(\d+)us", text)] if m), default=None)
    return underruns, late, worst, errors(rows)

def fields(line):
    head, _, message = line.partition(" message=")
    name, *pairs = head.split()
    return name, dict(pair.split("=", 1) for pair in pairs if "=" in pair) | {"message": message}
