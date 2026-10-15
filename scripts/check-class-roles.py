#!/usr/bin/env python3
"""One class name must not serve both a control and a container.

`settings-tab` was a rule written for a nav `<button>`: a nowrap, non-shrinking
flex row with a pointer cursor. A different page then put the same name on the
`<div>` wrapping a whole panel, and the unscoped rule turned that panel into a
horizontal row that could neither wrap nor shrink. The presence check
(`check-classes-styled.py`) passed throughout, because the class had a rule;
the rule just belonged to something else.

This flags any class that appears on a leaf control (`button`, `a`, `input`,
`select`, `textarea`, `label`) in one place and on a container (`div`,
`section`, `main`, `form`, ...) in another. It reads each start tag, including
ones whose attributes span lines. A class chosen by a helper function has no
literal to read and is not seen.

Usage: scripts/check-class-roles.py              (exits non-zero on a mixed-role class)
       scripts/check-class-roles.py --self-test  (proves it flags the original collision)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

CONTROLS = {"button", "a", "input", "select", "textarea", "label"}
CONTAINERS = {"div", "section", "main", "form", "article", "aside", "nav", "ul", "ol", "header", "footer"}

TAG_OPEN = re.compile(r"<([a-z][a-z0-9]*)\b")
# Any attribute that starts a line ends a multi-line class value.
NEXT_ATTR = re.compile(r"\n\s*[a-z][\w:-]*=(?!=)")


def class_literals(window: str) -> list[str]:
    """String literals of the first `class=` in a start tag's text, if any.

    Covers `class="a b"` and the multi-line `class=move || { if .. { "a" }
    else { "b" } }` form, which is how a nav button's class is written. The
    value ends at the next attribute that starts a line.
    """
    m = re.search(r"\bclass=", window)
    if not m:
        return []
    rest = window[m.end():]
    end = NEXT_ATTR.search(rest)
    if end:
        rest = rest[:end.start()]
    if rest.startswith('"'):
        close = rest.find('"', 1)
        return [rest[1:close]] if close > 0 else []
    return re.findall(r'"([^"\n]*)"', rest)


def roles(text: str) -> dict[str, dict[str, set[str]]]:
    """class -> {"control": {locations}, "container": {locations}}."""
    out: dict[str, dict[str, set[str]]] = {}
    for m in TAG_OPEN.finditer(text):
        tag = m.group(1)
        role = "control" if tag in CONTROLS else "container" if tag in CONTAINERS else None
        if role is None:
            continue
        line_start = text.rfind("\n", 0, m.start()) + 1
        if text[line_start:m.start()].lstrip().startswith("//"):
            continue
        nxt = re.compile(r"<[a-zA-Z/]").search(text, m.end())
        window = text[m.end():nxt.start() if nxt else len(text)]
        n = text.count("\n", 0, m.start()) + 1
        for value in class_literals(window):
            for tok in value.split():
                if "{" in tok or "}" in tok:
                    continue
                out.setdefault(tok, {"control": set(), "container": set()})[role].add(str(n))
    return out


def find_mixed(files: dict[str, str]) -> dict[str, dict[str, list[str]]]:
    merged: dict[str, dict[str, list[str]]] = {}
    for name, text in files.items():
        for cls, by_role in roles(text).items():
            slot = merged.setdefault(cls, {"control": [], "container": []})
            for role, lines in by_role.items():
                slot[role] += [f"{name}:{n}" for n in sorted(lines, key=int)]
    return {c: r for c, r in merged.items() if r["control"] and r["container"]}


# The shape of the original collision: a multi-line nav button class closure
# followed by an attribute outside any fixed list, and a panel div with the
# same name.
BAD_BUTTON = """
<button
    class=move || {
        if a < b { "settings-tab active".to_string() } else { "settings-tab".to_string() }
    }
    role="tab"
    on:click=f
>
"""
BAD_PANEL = '<div class="settings-tab"><p>x</p></div>'


def self_test() -> int:
    bad = find_mixed({"nav.rs": BAD_BUTTON, "panel.rs": BAD_PANEL})
    if "settings-tab" not in bad:
        print("self-test failed: the original collision was not flagged", file=sys.stderr)
        return 1
    ok = find_mixed({"nav.rs": BAD_BUTTON, "panel.rs": BAD_PANEL.replace("settings-tab", "store-settings")})
    if ok:
        print(f"self-test failed: false positive {sorted(ok)}", file=sys.stderr)
        return 1
    print("self-test ok: flags the original collision, clean once renamed")
    return 0


def main() -> int:
    if "--self-test" in sys.argv:
        return self_test()
    grep = subprocess.run(
        ["git", "grep", "-l", "class=", "--", "src/"],
        cwd=REPO, capture_output=True, text=True, check=False,
    )
    names = grep.stdout.split()
    if grep.returncode != 0 or not names:
        print(f"scanned nothing (git grep exit {grep.returncode}): {grep.stderr.strip()}", file=sys.stderr)
        return 2
    mixed = find_mixed({f: (REPO / f).read_text() for f in names})
    if not mixed:
        print("no class is used as both a control and a container")
        return 0
    print("classes used as both a control and a container:\n", file=sys.stderr)
    for cls, r in sorted(mixed.items()):
        print(f"  .{cls}", file=sys.stderr)
        print(f"    control:   {', '.join(r['control'])}", file=sys.stderr)
        print(f"    container: {', '.join(r['container'])}", file=sys.stderr)
    print(
        "\nA rule written for one will land on the other. Give the container "
        "its own class name.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
