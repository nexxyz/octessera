#!/usr/bin/env python3
import hashlib
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
UI_ROOTS = (
    ROOT / "userpatches/overlay/usr/local/share/octessera-setup-ui",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui",
)
UI_ACCOUNTS = ("octessera", "pi")
UI_HOSTNAMES = ("octessera-opi", "octessera-rpi")
EXPECTED_FILES = {"README.md", "index.html", "js/app.js", "css/styles.css", "img/octessera-mark.svg", "img/octessera-wordmark.svg"}
UI_FILES = [{path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()} for root in UI_ROOTS]
assert UI_FILES[0] == UI_FILES[1] == EXPECTED_FILES
SHARED_UI_BYTES = {
    relative: [
        (root / relative)
        .read_bytes()
        .replace(f"<code>{account}</code>".encode(), b"<code>ACCOUNT</code>")
        .replace(hostname.encode(), b"HOSTNAME")
        if relative == "index.html"
        else (root / relative).read_bytes()
        for root, account, hostname in zip(UI_ROOTS, UI_ACCOUNTS, UI_HOSTNAMES)
    ]
    for relative in sorted(EXPECTED_FILES)
}
for relative, contents in SHARED_UI_BYTES.items():
    assert contents[0] == contents[1], relative
    assert len({hashlib.sha256(content).hexdigest() for content in contents}) == 1, relative
APP_BYTES = SHARED_UI_BYTES["js/app.js"]
SHARED_IDS = set(re.findall(rb"getElementById\('([^']+)'\)", APP_BYTES[0]))


for root, account, hostname in zip(UI_ROOTS, UI_ACCOUNTS, UI_HOSTNAMES):
    html = (root / "index.html").read_text(encoding="utf-8")
    assert f'<span id="localPasswordTitle">Local device password for user <code>{account}</code></span>' in html
    assert f'placeholder="{hostname}"' in html and "octessera-device" not in html
    html_ids = set(re.findall(r'id="([^"]+)"', html))
    references = re.findall(r'(?:href|src)="(/[^"?#]+)', html)
    route_prefixes = {
        "/css/": root / "css",
        "/js/": root / "js",
        "/img/": root / "img",
    }
    for reference in references:
        prefix = next((prefix for prefix in route_prefixes if reference.startswith(prefix)), None)
        assert prefix is not None
        assert (route_prefixes[prefix] / reference.removeprefix(prefix)).is_file()
    assert {value.decode("utf-8") for value in SHARED_IDS} <= html_ids

print("Setup UI parity and resource reference tests passed")
