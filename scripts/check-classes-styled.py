#!/usr/bin/env python3
"""Every class the markup emits must have a rule somewhere.

This exists because a rename shipped without it. `btn-sm` became `ps-btn-sm`
across 69 call sites, the rule stayed behind as `.btn-sm`, and 104 buttons lost
their styling on testnet. Nothing caught it:

* the layout suite passes, because it asserts geometry, not colour or size
* the e2e suite passes, because it locates elements by class and the classes are
  all present - just unstyled
* checking that the stylesheet's selector set was preserved passes, because it
  was: the rules still exist, under names nothing renders any more

That last one is the trap. "No selector was lost" and "every emitted class is
styled" sound like the same check and are not. This is the second one.

It used to check only `ps-` classes, and that scoping cost something. The
plugin page rendered into `class="page"` - a class this stylesheet has never
defined - so it had no max width, no column, no gap and no heading while every
screen beside it had all three. Unprefixed, so nothing looked. Generalising it
turned up 78 more, including every class on the 404 page and the whole of
`LoadingState`, which renders two empty divs and a paragraph on the invoice
list and detail pages.

The old scoping was for a real reason - noise. Reading every double-quoted
string in a file finds "the", "and" and "you" and calls them classes. So this
reads only the values of `class=` attributes: the literal when the attribute is
one, and every literal in the expression when it is a conditional, which is how
`ps-btn-success` and `ps-btn-warning` stay covered. 510 classes, one false
positive.

What it cannot see is a class chosen by a function - `class=tone_class(tone)`
has no literal on the line. `plugin_page.rs` covers its own four such helpers
in a unit test, by calling them. That unit test is the only thing standing
behind them, so a new helper that is not added to it is unguarded by anything:
`figure_class` was added without being listed there and shipped a tone class
with no rule, green through both checks.

## The baseline

Generalising found 78 unstyled classes that predate it, on pages nobody is
changing today. Failing on all of them would mean either a red CI nobody can
fix in one sitting, or a check nobody turns on. So `classes-unstyled-baseline.txt`
records them, this fails on anything new, and it *also* fails when a baselined
class becomes styled without being removed from the list - so the list only
shrinks.

Usage: scripts/check-classes-styled.py   (exits non-zero on an unstyled class)
       scripts/check-classes-styled.py --update-baseline
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent


BASELINE = Path(__file__).resolve().parent / "classes-unstyled-baseline.txt"


def class_attribute_values(line: str) -> list[str]:
    """Every string literal that forms part of a `class=` attribute value.

    Two shapes, because Leptos writes both:

      class="a b"                       -> the literal
      class=if c { "a" } else { "b" }   -> both branches

    The second is why reading only the first literal after `class=` is not
    enough: that is how `ps-btn-warning` hid from an earlier version of this
    script. The first is why reading to the end of the line is too much: a line
    can carry `style="..."` and text content as well, and counting those finds
    470 "classes" where there are 510 real ones.
    """
    out: list[str] = []
    for m in re.finditer(r"class=", line):
        rest = line[m.end():]
        if rest.startswith('"'):
            end = rest.find('"', 1)
            if end > 0:
                out.append(rest[1:end])
        else:
            out.extend(re.findall(r'"([^"\n]*)"', rest))
    return out


def emitted_classes() -> dict[str, int]:
    """Every class token the markup emits, with a use count."""
    files = subprocess.run(
        ["git", "grep", "-l", "class=", "--", "src/"],
        cwd=REPO, capture_output=True, text=True, check=False,
    ).stdout.split()
    found: dict[str, int] = {}
    for f in files:
        for line in (REPO / f).read_text().splitlines():
            # A commented-out line is not markup, and the comments in this
            # codebase quote class names while explaining them.
            if line.lstrip().startswith("//"):
                continue
            for value in class_attribute_values(line):
                for tok in value.split():
                    # A class interpolated from a variable leaves `{}` behind.
                    # It is not a name, so there is nothing to look up.
                    if "{" in tok or "}" in tok:
                        continue
                    found[tok] = found.get(tok, 0) + 1
    return found


def baseline() -> set[str]:
    if not BASELINE.exists():
        return set()
    return {
        line.strip()
        for line in BASELINE.read_text().splitlines()
        if line.strip() and not line.startswith("#")
    }


def uikit_css() -> str:
    """ui-kit's own stylesheet, located through cargo rather than a guessed path.

    It lives in a git dependency, so its on-disk location is a cargo checkout
    hash, not a sibling directory. `cargo metadata` is the only reliable way to
    find it, and it works in CI where the dependency is fetched but no commons
    checkout exists.
    """
    import json
    try:
        meta = json.loads(subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            cwd=REPO, capture_output=True, text=True, check=True,
        ).stdout)
    except (subprocess.CalledProcessError, FileNotFoundError, json.JSONDecodeError):
        return ""
    for pkg in meta.get("packages", []):
        if pkg.get("name") == "ui-kit":
            css = Path(pkg["manifest_path"]).parent / "styles" / "ui-kit.css"
            if css.exists():
                return css.read_text()
    return ""


def styled_classes() -> set[str]:
    """Classes with at least one rule, across both stylesheets."""
    css = (REPO / "styles.css").read_text()
    extra = uikit_css()
    if extra:
        css += extra
    else:
        print("warning: ui-kit's stylesheet could not be located through cargo; "
              "classes styled only by ui-kit may be reported as unstyled",
              file=sys.stderr)
    # Comments mention class names in prose. Counting those as rules is exactly
    # how a missing rule would hide.
    css = re.sub(r"/\*.*?\*/", "", css, flags=re.S)
    return set(re.findall(r"\.([a-zA-Z][\w-]*)", css))


def main() -> int:
    emitted = emitted_classes()
    styled = styled_classes()
    missing = {c: n for c, n in emitted.items() if c not in styled}

    if "--update-baseline" in sys.argv:
        BASELINE.write_text(
            "# Classes the markup emits that no rule matches.\n"
            "# Each one renders with no styling. This file exists so the check\n"
            "# could be switched on without fixing all of them at once; it is\n"
            "# meant to shrink. Do not add to it to make a build pass.\n"
            "# Regenerate: scripts/check-classes-styled.py --update-baseline\n"
            + "".join(f"{c}\n" for c in sorted(missing))
        )
        print(f"baseline written with {len(missing)} unstyled class(es)")
        return 0

    known = baseline()
    new = {c: n for c, n in missing.items() if c not in known}
    # A baselined class that now has a rule must leave the list, or the list
    # stops meaning anything and starts hiding the next regression under the
    # same name.
    fixed = sorted(known - set(missing))

    if not new and not fixed:
        print(
            f"all {len(emitted)} classes emitted by the markup have a rule, "
            f"except {len(known)} known unstyled one(s)"
        )
        return 0

    if new:
        print("unstyled classes in rendered markup:\n", file=sys.stderr)
        for c, n in sorted(new.items(), key=lambda kv: -kv[1]):
            print(f"  {c:<28} {n:>4} use(s)", file=sys.stderr)
        print(
            "\nEach of these renders with no styling. Either add a rule, or point "
            "the markup at a class that has one.",
            file=sys.stderr,
        )

    if fixed:
        print(
            "\nthese are styled now and are still listed as unstyled:\n",
            file=sys.stderr,
        )
        for c in fixed:
            print(f"  {c}", file=sys.stderr)
        print(
            "\nRemove them from scripts/classes-unstyled-baseline.txt - the list is "
            "meant to shrink, and one that does not stops meaning anything.",
            file=sys.stderr,
        )

    return 1


if __name__ == "__main__":
    sys.exit(main())
