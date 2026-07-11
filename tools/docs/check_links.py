from __future__ import annotations

import argparse
import re
import ssl
import sys
import urllib.error
import urllib.request
from pathlib import Path


EXCLUDED_PARTS = {".git", ".slim", "node_modules", "target", "dist", "dist-desktop"}
LOCAL_LINK_RE = re.compile(r"(?<!\!)\[[^\]]+\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|<((?:\.\.?/|/)[^>]+)>")
HTTP_LINK_RE = re.compile(r"(?<!\!)\[[^\]]+\]\((https?://[^)\s]+)(?:\s+\"[^\"]*\")?\)|<(https?://[^>]+)>")
FENCE_RE = re.compile(r"```.*?```", re.DOTALL)

EXPECTED_HTTP_SNIPPETS = {
    "https://www.adafruit.com/product/1431": ["1431", "ssd1351"],
    "https://www.adafruit.com/product/1611": ["1611", "silicone"],
    "https://www.adafruit.com/product/3954": ["3954", "neotrellis"],
    "https://www.adafruit.com/product/4090": ["4090", "usb"],
    "https://www.adafruit.com/product/4401": ["4401", "stemma"],
    "https://www.adafruit.com/product/4980": ["4980", "neokey"],
    "https://www.adafruit.com/product/6250": ["6250", "pcm510"],
    "https://www.amazon.de/-/en/CHERRY-Mechanical-Keyboard-Switches-without/dp/B0CBS4HJJR?th=1": ["cherry"],
}


def main() -> int:
    parser = argparse.ArgumentParser(description="Check Markdown local links and optional HTTP product content.")
    parser.add_argument("paths", nargs="*", default=["."], help="Files or directories to scan.")
    parser.add_argument("--http", action="store_true", help="Fetch HTTP links and fail clear 404/410 or expected-content mismatches.")
    parser.add_argument("--strict-http", action="store_true", help="Fail HTTP warnings such as timeouts or 403 blocks.")
    args = parser.parse_args()

    root = Path.cwd().resolve()
    md_files = markdown_files(root, [Path(path) for path in args.paths])
    broken_local = check_local_links(root, md_files)
    http_failures: list[str] = []
    http_warnings: list[str] = []
    if args.http:
        http_failures, http_warnings = check_http_links(md_files)

    for line in broken_local:
        print(f"BROKEN local {line}")
    for line in http_failures:
        print(f"BROKEN http {line}")
    for line in http_warnings:
        print(f"WARN http {line}")

    print(
        f"checked_files={len(md_files)} broken_local={len(broken_local)} "
        f"http_failures={len(http_failures)} http_warnings={len(http_warnings)}"
    )
    if broken_local or http_failures or (args.strict_http and http_warnings):
        return 1
    return 0


def markdown_files(root: Path, inputs: list[Path]) -> list[Path]:
    files: list[Path] = []
    for input_path in inputs:
        path = (root / input_path).resolve()
        if path.is_file() and path.suffix == ".md":
            files.append(path)
        elif path.is_dir():
            files.extend(
                child
                for child in path.rglob("*.md")
                if not any(part in EXCLUDED_PARTS for part in child.relative_to(root).parts)
            )
    return sorted(set(files))


def markdown_without_fences(path: Path) -> str:
    return FENCE_RE.sub("", path.read_text(encoding="utf-8", errors="ignore"))


def check_local_links(root: Path, md_files: list[Path]) -> list[str]:
    broken: list[str] = []
    for md_file in md_files:
        text = markdown_without_fences(md_file)
        for match in LOCAL_LINK_RE.finditer(text):
            raw = (match.group(1) or match.group(2) or "").strip()
            if not raw or raw.startswith(("http://", "https://", "mailto:", "#", "file:")):
                continue
            target = raw.split("#", 1)[0]
            if not target:
                continue
            target_path = (md_file.parent / target.replace("%20", " ")).resolve()
            if target.startswith("/"):
                target_path = (root / target.lstrip("/")).resolve()
            if not target_path.exists():
                line = text.count("\n", 0, match.start()) + 1
                broken.append(f"{md_file.relative_to(root).as_posix()}:{line}: {raw}")
    return broken


def check_http_links(md_files: list[Path]) -> tuple[list[str], list[str]]:
    refs: dict[str, list[str]] = {}
    root = Path.cwd().resolve()
    for md_file in md_files:
        text = markdown_without_fences(md_file)
        for match in HTTP_LINK_RE.finditer(text):
            url = (match.group(1) or match.group(2) or "").strip()
            line = text.count("\n", 0, match.start()) + 1
            refs.setdefault(url, []).append(f"{md_file.relative_to(root).as_posix()}:{line}")

    failures: list[str] = []
    warnings: list[str] = []
    context = ssl.create_default_context()
    for url, locations in sorted(refs.items()):
        try:
            body = fetch_text(url, context)
        except urllib.error.HTTPError as error:
            message = f"{error.code} {url} ({', '.join(locations)})"
            if error.code in (404, 410):
                failures.append(message)
            else:
                warnings.append(message)
            continue
        except Exception as error:  # noqa: BLE001 - diagnostic tool should report all fetch errors.
            warnings.append(f"{type(error).__name__} {url}: {str(error)[:140]} ({', '.join(locations)})")
            continue

        snippets = EXPECTED_HTTP_SNIPPETS.get(url)
        if snippets and not all(snippet.lower() in body.lower() for snippet in snippets):
            failures.append(f"missing expected content {snippets!r} {url} ({', '.join(locations)})")
        if "can't find the page" in body.lower() or "can’t find the page" in body.lower():
            failures.append(f"not-found text {url} ({', '.join(locations)})")
    return failures, warnings


def fetch_text(url: str, context: ssl.SSLContext) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 link-check"})
    with urllib.request.urlopen(request, timeout=20, context=context) as response:
        data = response.read().decode("utf-8", errors="ignore")
    return re.sub(r"\s+", " ", re.sub(r"<[^>]+>", " ", data))


if __name__ == "__main__":
    raise SystemExit(main())
