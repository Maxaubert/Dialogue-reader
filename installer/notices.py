"""Writes the Rust crate list of THIRD_PARTY_NOTICES.md: every crate linked into DialogReader.exe, its licence and the
copyright lines of its own licence files.

    python installer/notices.py           # rewrite the generated part of THIRD_PARTY_NOTICES.md
    python installer/notices.py --check   # exit 1 when the file is out of date (Cargo.lock changed)

The crates are the normal dependencies of dr-app for x86_64-pc-windows-msvc (`cargo tree -e normal`): no build or
dev dependencies and no proc-macros, which do not end up in the exe. Copyright lines are read from the crate's
LICENSE*, COPYING*, COPYRIGHT* and NOTICE* files in the cargo registry; a crate whose licence files have none is
shown with its Cargo.toml authors. Only the part between the two markers is written; the hand-written parts of the
file (Kokoro, Slint, Skia, fonts, the files next to the exe, licence texts) stay as they are.
Stdlib only (2026-11-23, M5).
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
NOTICES = REPO / "THIRD_PARTY_NOTICES.md"
BEGIN = "<!-- crates:begin (installer/notices.py writes this part) -->"
END = "<!-- crates:end -->"
TARGET = "x86_64-pc-windows-msvc"
LICENCE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|COPYRIGHT|NOTICE)", re.I)
# a copyright line, not licence boilerplate that mentions the word
COPYRIGHT = re.compile(r"^\s*(copyright\s*(\(c\)|©|\d{4})|\(c\)\s*\d{4}|©\s*\d{4})", re.I)
BOILERPLATE = re.compile(r"\[yyyy\]|\{yyyy\}|<year>|\[name of copyright owner\]|copyright notice", re.I)


def run(args: list[str]) -> str:
    return subprocess.run(args, cwd=REPO, check=True, capture_output=True, text=True, encoding="utf-8").stdout


def shipped() -> list[tuple[str, str]]:
    """(name, version) of every crate in the exe, our own crates left out."""
    out = run(["cargo", "tree", "-p", "dr-app", "--target", TARGET, "-e", "normal", "--prefix", "none",
               "--format", "{p}"])
    seen = set()
    for line in out.splitlines():
        line = line.replace(" (*)", "").strip()
        if not line or "(proc-macro)" in line or "dialogue-reader-rs" in line.replace("\\", "/"):
            continue
        name, ver = line.split()[:2]
        seen.add((name, ver.lstrip("v")))
    return sorted(seen, key=lambda p: (p[0].lower(), p[1]))


def packages() -> dict[tuple[str, str], dict]:
    meta = json.loads(run(["cargo", "metadata", "--format-version", "1", "--locked"]))
    return {(p["name"], p["version"]): p for p in meta["packages"]}


def copyright_lines(folder: Path) -> list[str]:
    lines: list[str] = []
    for f in sorted(folder.iterdir()):
        if not f.is_file() or not LICENCE_FILE.match(f.name):
            continue
        try:
            text = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for raw in text.splitlines():
            line = " ".join(raw.strip().strip("#*/").split())
            if COPYRIGHT.match(line) and not BOILERPLATE.search(line) and len(line) < 200:
                if line not in lines:
                    lines.append(line)
    return lines


def cell(text: str) -> str:
    return text.replace("|", "\\|")


def table() -> str:
    pkgs = packages()
    rows = ["| Crate | Licence | Copyright |", "|---|---|---|"]
    for name, ver in shipped():
        p = pkgs.get((name, ver))
        if p is None:
            raise SystemExit(f"{name} {ver} is not in cargo metadata")
        lic = p.get("license") or (f"see {p['license_file']}" if p.get("license_file") else "not stated")
        folder = Path(p["manifest_path"]).parent
        lines = copyright_lines(folder)
        if lines:
            c = "; ".join(lines)
        else:
            authors = ", ".join(a.split("<")[0].strip() for a in p.get("authors", []) if a.strip())
            c = f"no copyright line in its licence files; authors: {authors}" if authors else \
                "no copyright line in its licence files"
        rows.append(f"| {name} {ver} | {cell(lic)} | {cell(c)} |")
    return "\n".join(rows)


def main() -> int:
    text = NOTICES.read_text(encoding="utf-8")
    if BEGIN not in text or END not in text:
        raise SystemExit(f"{NOTICES.name} has no {BEGIN} ... {END} markers")
    head, rest = text.split(BEGIN, 1)
    _, tail = rest.split(END, 1)
    new = f"{head}{BEGIN}\n{table()}\n{END}{tail}"
    if "--check" in sys.argv:
        if new != text:
            print(f"{NOTICES.name} is out of date: run python installer/notices.py", file=sys.stderr)
            return 1
        return 0
    NOTICES.write_text(new, encoding="utf-8", newline="\n")
    print(f"wrote {NOTICES.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
