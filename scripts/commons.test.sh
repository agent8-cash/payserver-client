#!/usr/bin/env bash
# Guard for scripts/commons.sh. Runs the real script against a small fixture
# manifest, so it never reads or copies the real repo and never reaches the
# network. The fixture's crate set is written out here as the expectation; the
# script must derive its own set from the manifest and match it.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

# Offline, so `link`'s trailing `cargo metadata` fails fast instead of fetching.
export CARGO_NET_OFFLINE=true

check() { # desc expected_rc [detail]
  local desc="$1" want="$2" detail="${3:-}"
  if [ "$want" -eq 0 ]; then
    echo "ok: $desc"; pass=$((pass+1))
  else
    echo "FAIL: $desc"; [ -n "$detail" ] && echo "    $detail"; fail=$((fail+1))
  fi
}

# The commons URL is whatever the real manifest pins, so this file holds no copy.
url="$(grep -m1 -oP 'git = "\K[^"]+' "$HERE/Cargo.toml")"
[ -n "$url" ] || { echo "FAIL: real Cargo.toml pins no git URL"; exit 1; }

EXPECTED=(api-types crypto payserver-plugin-api scrub types ui-kit)
rev="$(printf '1%.0s' {1..40})"

# Fixture repo: the real script, plus a manifest declaring EXPECTED crates.
make_repo() { # dir [crate...]
  local dir="$1"; shift
  mkdir -p "$dir/scripts"
  cp "$HERE/scripts/commons.sh" "$dir/scripts/commons.sh"
  {
    echo '[package]'; echo 'name = "fixture"'; echo 'version = "0.0.0"'; echo 'edition = "2021"'
    echo '[dependencies]'
    for c in "$@"; do echo "$c = { git = \"$url\", rev = \"$rev\" }"; done
  } > "$dir/Cargo.toml"
}

REPO="$TMP/repo"
make_repo "$REPO" "${EXPECTED[@]}"
COMMONS="$TMP/commons"
for c in "${EXPECTED[@]}"; do mkdir -p "$COMMONS/$c"; done
cd "$REPO"

# link: exits 0 and patches exactly the expected set
bash scripts/commons.sh link "$COMMONS" >/dev/null 2>&1; rc=$?
check "link exits 0 on the fixture manifest" "$rc"

patched="$(grep -oP '^[a-z0-9_-]+(?= *= *\{ *path)' .cargo/config.toml 2>/dev/null | sort -u || true)"
want="$(printf '%s\n' "${EXPECTED[@]}" | sort -u)"
if [ "$patched" = "$want" ]; then
  check "link patches exactly the manifest's crates" 0
else
  check "link patches exactly the manifest's crates" 1 "patched: $(echo $patched) / expected: $(echo $want)"
fi
for c in "${EXPECTED[@]}"; do
  if grep -qP "^$c = \{ path = " .cargo/config.toml 2>/dev/null; then
    check "link patches $c" 0
  else
    check "link patches $c" 1
  fi
done

# unlink: removes the patch and rewrites the lock with the same crate set dropped
{
  for c in "${EXPECTED[@]}"; do printf '[[package]]\nname = "%s"\nversion = "0.0.0"\n\n' "$c"; done
  printf '[[package]]\nname = "unrelated"\nversion = "0.0.0"\n'
} > Cargo.lock
bash scripts/commons.sh unlink >/dev/null 2>&1; rc=$?
check "unlink exits 0" "$rc"
[ ! -f .cargo/config.toml ] && check "unlink removes the patch config" 0 || check "unlink removes the patch config" 1
stale="$(grep -oP '^name = "\K[^"]+' Cargo.lock | grep -xF -f <(printf '%s\n' "${EXPECTED[@]}") || true)"
[ -z "$stale" ] && check "unlink drops every expected crate from Cargo.lock" 0 \
  || check "unlink drops every expected crate from Cargo.lock" 1 "still present: $stale"
grep -q '^name = "unrelated"' Cargo.lock && check "unlink keeps unrelated lock entries" 0 \
  || check "unlink keeps unrelated lock entries" 1

# Drift: a crate added to the manifest later, absent from EXPECTED, must still be
# patched. A literal list in the script would fail this.
DRIFT="$TMP/drift"
make_repo "$DRIFT" "${EXPECTED[@]}" "fixture-added"
mkdir -p "$COMMONS/fixture-added"
cd "$DRIFT"
bash scripts/commons.sh link "$COMMONS" >/dev/null 2>&1
grep -qP '^fixture-added = \{ path = ' .cargo/config.toml 2>/dev/null \
  && check "link patches a crate added to the manifest later" 0 \
  || check "link patches a crate added to the manifest later" 1

# The real manifest: link patches every commons crate it pins, no more, no fewer.
REAL="$TMP/real"; mkdir -p "$REAL/scripts"
cp "$HERE/scripts/commons.sh" "$REAL/scripts/"; cp "$HERE/Cargo.toml" "$REAL/"
real_crates="$(grep -F "git = \"$url\"" "$HERE/Cargo.toml" | awk '{print $1}' | sort -u)"
[ -n "$real_crates" ] || { echo "FAIL: real Cargo.toml pins no commons crate"; exit 1; }
for c in $real_crates; do mkdir -p "$COMMONS/$c"; done
cd "$REAL"
bash scripts/commons.sh link "$COMMONS" >/dev/null 2>&1; rc=$?
check "link exits 0 on the real manifest" "$rc"
real_patched="$(grep -oP '^[a-z0-9_-]+(?= *= *\{ *path)' .cargo/config.toml 2>/dev/null | sort -u || true)"
if [ "$real_patched" = "$real_crates" ]; then
  check "link patches exactly the real manifest's commons crates" 0
else
  check "link patches exactly the real manifest's commons crates" 1 "patched: $(echo $real_patched) / pinned: $(echo $real_crates)"
fi
for c in $real_crates; do rm -rf "$COMMONS/$c"; done

# Read-only subcommands work on a manifest the detection does not match.
REPO_EMPTY="$TMP/empty"
make_repo "$REPO_EMPTY"
cd "$REPO_EMPTY"
bash scripts/commons.sh status >/dev/null 2>&1; check "status exits 0 with no commons crates" $?
bash scripts/commons.sh --help >/dev/null 2>&1; check "--help exits 0 with no commons crates" $?
set +e; bash scripts/commons.sh bogus >/dev/null 2>&1; rc=$?; set -e
[ "$rc" -ne 0 ] && check "unknown subcommand still exits non-zero" 0 || check "unknown subcommand still exits non-zero" 1
set +e; bash scripts/commons.sh link "$COMMONS" >/dev/null 2>&1; rc=$?; set -e
[ "$rc" -ne 0 ] && check "link refuses a manifest with no commons crates" 0 \
  || check "link refuses a manifest with no commons crates" 1

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
