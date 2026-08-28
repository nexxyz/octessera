#!/usr/bin/env python3
import hashlib
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
UI_ROOTS = (
    ROOT / "userpatches/overlay/usr/local/share/octessera-setup-ui",
    ROOT / "tools/pi-image/stage4-octessera/files/root/usr/local/share/octessera-setup-ui",
)
EXPECTED_FILES = {"README.md", "index.html", "js/app.js", "css/styles.css", "img/octessera-mark.svg", "img/octessera-wordmark.svg"}
UI_FILES = [{path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file()} for root in UI_ROOTS]
assert UI_FILES[0] == UI_FILES[1] == EXPECTED_FILES
SHARED_UI_BYTES = {relative: [(root / relative).read_bytes() for root in UI_ROOTS] for relative in sorted(EXPECTED_FILES)}
for relative, contents in SHARED_UI_BYTES.items():
    assert contents[0] == contents[1], relative
    assert len({hashlib.sha256(content).hexdigest() for content in contents}) == 1, relative
APP_BYTES = SHARED_UI_BYTES["js/app.js"]
SHARED_IDS = set(re.findall(rb"getElementById\('([^']+)'\)", APP_BYTES[0]))


for root in UI_ROOTS:
    app = (root / "js/app.js").read_text(encoding="utf-8")
    html = (root / "index.html").read_text(encoding="utf-8")
    readme = (root / "README.md").read_text(encoding="utf-8")
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
    assert "country" in app and "/country" in app
    assert "state.networks = [];" not in app
    assert "First boot setup" not in html and "Armbian" not in html
    assert "Wi-Fi &amp; access setup" in html
    assert "aria-busy=\"false\"" in html
    styles = (root / "css/styles.css").read_text(encoding="utf-8")
    assert "progress-track" not in html + app and "--progress" not in html + app + styles
    for obsolete in ("data-step", "data-next", "data-back", "progress", "stepDots", "review", "setStep", "goNext", "goBack", "renderReview", "Welcome"):
        assert obsolete not in html and obsolete not in app
    assert html.count("<form") == 1 and "Apply setup" in html
    assert "id=\"applyingPanel\"" in html and "id=\"applyingStatus\"" in html
    assert "html, body { width: 100%; margin: 0; min-height: 100%; }" in styles
    assert ".shell {\n  width: calc(100% - 1rem);\n  max-width: 900px;" in styles
    assert "width: min(100vw - 0.75rem, 900px)" not in styles
    assert ".network-item {\n  position: relative;\n  display: grid;\n  grid-template-columns: 1.25rem minmax(0, 1fr);\n  gap: 0.75rem;\n  align-items: start;\n}" in styles
    assert ".network-radio {\n  position: absolute;" in styles
    assert "clip-path: inset(50%);" in styles
    assert ".network-radio-mark {\n  display: grid;" in styles
    assert ".network-radio:checked + .network-radio-mark::after { transform: scale(1); }" in styles
    assert ".network-radio:focus-visible + .network-radio-mark {" in styles
    assert ".network-item.is-selected {" in styles
    assert ".choice > span { min-width: 0; overflow-wrap: anywhere; }" in styles
    assert ".network-copy { display: block; min-width: 0; overflow: hidden; }" in styles
    assert ".network-meta { display: block; max-width: 100%;" in styles
    assert ".field-header > * { min-width: 0; }" in styles
    assert ".field-header .link { width: 100%; }" in styles
    assert "grid-template-columns: minmax(0, 1fr);" in styles and "gap: 0.55rem;" in styles
    assert ".choice { width: 100%; min-width: 0; min-height: 44px; }" in styles
    assert ".field input, .field textarea {\n  width: 100%;\n  min-width: 0;\n  max-width: 100%;" in styles
    assert ".shell { width: calc(100% - 0.5rem); }" in styles
    assert ".network-item { gap: 0.6rem; padding: 0.7rem; }" in styles
    assert "min-height: 52px" in styles and "min-height: 44px" in styles
    assert "-webkit-line-clamp: 2" in styles and "min-width: 0" in styles
    assert "@media (max-width: 520px)" in styles
    assert "@media (max-width: 390px)" in styles
    assert "@media (max-width: 360px)" in styles
    assert "focus-visible" in styles
    assert "els.refreshNetworks.disabled = loading" in app
    assert "state.networkPhase = 'scan'" in app
    assert "manualSsid" in html and "manually" in app
    assert "title=\"${escapeHtml(network.ssid)}\"" in app
    assert "aria-label=\"Select ${escapeHtml(network.ssid)}\"" in app
    assert "class=\"network-radio\" type=\"radio\" name=\"ssidChoice\"" in app
    assert "class=\"network-radio\"" in app and "class=\"network-radio-mark\" aria-hidden=\"true\"" in app
    assert "!state.manualSsid && state.selectedSsid === network.ssid ? ' is-selected' : ''" in app
    assert "const effectiveSsid = () => state.manualSsid || state.selectedSsid;" in app
    assert "if (!effectiveSsid())" in app and "ssid: effectiveSsid()," in app
    selection_start = app.index("if (target.name === 'ssidChoice') {")
    selection_end = app.index("render();", selection_start)
    selection = app[selection_start:selection_end]
    assert selection.index("els.manualSsid.value = '';") < selection.index("state.manualSsid = '';") < selection.index("state.selectedSsid = target.value;")
    assert app.index("els.manualSsid.value = '';", selection_start) < selection_end
    assert "const clearScannedSelection = () =>" in app
    assert "input.checked = false;" in app
    assert "if (event.target.id === 'manualSsid' && state.manualSsid) {\n      clearScannedSelection();" in app
    assert "if (event.target.matches('input[name=\"ssidChoice\"]')) {\n      return;\n    }\n    syncStateFromInputs();" in app
    assert "const focusSelectedNetwork = () =>" in app
    assert "Array.from(els.networkList.querySelectorAll('input[name=\"ssidChoice\"]')).find((input) => input.value === state.selectedSsid)" in app
    assert "radio.focus({ preventScroll: true });" in app and "radio.focus();" in app
    scanned_change = app.index("if (target.name === 'ssidChoice') {")
    assert app.index("render();\n      focusSelectedNetwork();", scanned_change) < app.index("return;", scanned_change)
    assert app.count("focusSelectedNetwork();") == 1
    for text in ("Locked", "Open", "Strong", "Fair", "Weak", " signal"):
        assert text in app
    assert "minlength=\"8\"" in html
    assert "state.sshPassword.length < 8" in app
    assert "SSH passwords need at least 8 characters." in app
    assert "12 characters" not in app and "minlength=\"12\"" not in html
    assert ".focus()" in app and "invalid.field" in app
    order = [
        html.index('id="wifiCountry"'),
        html.index('id="networkList"'),
        html.index('id="manualSsid"'),
        html.index('id="wifiPassphrase"'),
        html.index('name="sshMode"'),
        html.index('id="hostname"'),
        html.index('id="applyButton"'),
    ]
    assert order == sorted(order)
    network = min(index for marker in ("await fetch(API.networks", "await fetch('/networks'") if (index := app.find(marker)) >= 0)
    assert app.index("await fetch(API.country") < network
    stage = app.index("fetch(API.stage")
    connect = min(index for marker in ("fetch(API.connect", "fetch('/connect'") if (index := app.find(marker)) >= 0)
    applying = app.index("els.form.hidden = true")
    assert stage < applying < connect
    assert app.count("fetch(API.connect") == 1
    assert "const stageFailureMessage = async (response) =>" in app
    assert "if (response.status !== 400)" in app
    assert "const payload = await response.json();" in app
    assert "payload?.error === 'invalid_input'" in app
    assert "els.errors.textContent = await stageFailureMessage(stageResponse);" in app
    assert "Setup stage failed (HTTP ${response.status})." in app
    for text in (
        "Applying Wi-Fi country…",
        "Scanning for nearby Wi-Fi networks…",
        "Enter a two-letter Wi-Fi country code before scanning.",
        "Could not apply the Wi-Fi country. Check the code and try again.",
        "Setup is being applied. Watch the OLED for the final result.",
        "The portal connection may have dropped while setup applies.",
        "The device rejected the Wi-Fi request",
    ):
        assert text in app
    for text in (
        "Applying setup",
        "Watch the OLED for the final result.",
        "After success, find the address in",
        "System &gt; Info",
    ):
        assert text in html
    for text in ("Setup request sent", "Load failed", "authoritative", "success screen"):
        assert text not in app and text not in html
    assert "Apply setup" in html
    assert "octessera-mark.svg" in html and "octessera-wordmark.svg" in html
    assert '<meta name="color-scheme" content="dark" />' in html
    assert "POST http://192.168.42.1:8080/country" in readme
    assert "POST http://192.168.42.1:8080/stage" in readme
    assert "GET /networks" in readme and "wifi-connect" in readme
    assert "POST /connect" in readme
    assert "the root coordinator in memory" in readme
    assert "provisional `Applying setup`" in readme
    assert "browser makes no completion or retry call" in readme
    for route in ("/finalize", "/discard", "/complete", "/retry"):
        assert route not in app and route not in html
    assert "authoritative" not in readme

print("Setup UI exact parity, accessibility, country order, applying flow, copy, and boundary tests passed")
