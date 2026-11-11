#!/usr/bin/env bash
# Test that commons.sh link patches all crates listed in the manifest.
# Regression test for AG8-1050: a stale hardcoded crate list missed
# payserver-plugin-api and crypto, so `link` reported success while
# silently keeping those crates on the pinned revision instead of the
# local checkout.
set -euo pipefail

SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/commons.sh"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

check() { # desc expected_rc [detail]
  local desc="$1" want="$2" detail="${3:-}"
  if [ "$want" -eq 0 ]; then
    echo "ok: $desc"
    pass=$((pass+1))
  else
    echo "FAIL: $desc"
    [ -n "$detail" ] && echo "    $detail"
    fail=$((fail+1))
  fi
}

# Copy the repo to a temp space so we can modify Cargo.toml without
# affecting the real one.
HERE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cp -r "$HERE_DIR" "$TMP/test_repo"
TEST_REPO="$TMP/test_repo"
COMMONS_SH="$TEST_REPO/scripts/commons.sh"

# Extract crates from manifest by running the same logic as commons.sh
get_detected_crates() {
  local manifest="$1" url="https://github.com/randomcash/payserver-commons.git"
  grep -oP '^\K[a-z0-9_-]+(?= *= *\{ *git *= *"'"$url"'")' "$manifest" | sort -u
}

# Get the crates the manifest actually declares
MANIFEST_CRATES=$(get_detected_crates "$TEST_REPO/Cargo.toml")
MANIFEST_COUNT=$(echo "$MANIFEST_CRATES" | wc -w)

# Verify the manifest has at least 6 crates (the current set)
if [ "$MANIFEST_COUNT" -ge 6 ]; then
  check "manifest declares at least 6 crates" 0
else
  check "manifest declares at least 6 crates" 1 "found only $MANIFEST_COUNT: $MANIFEST_CRATES"
fi

# Verify that api-types, scrub, types, ui-kit (the old hardcoded set)
# plus payserver-plugin-api and crypto are all present
for crate in api-types scrub types ui-kit payserver-plugin-api crypto; do
  if echo "$MANIFEST_CRATES" | grep -q "^$crate$"; then
    check "manifest includes $crate" 0
  else
    check "manifest includes $crate" 1
  fi
done

# Create a fake commons checkout in temp
FAKE_COMMONS="$TMP/fake-commons"
mkdir -p "$FAKE_COMMONS"
for crate in $MANIFEST_CRATES; do
  mkdir -p "$FAKE_COMMONS/$crate"
done

# Test that `link` patches all crates from the manifest
cd "$TEST_REPO"
bash "$COMMONS_SH" link "$FAKE_COMMONS" >/dev/null 2>&1

# Count patches in the generated config
if [ -f .cargo/config.toml ]; then
  PATCHED_CRATES=$(grep -oP "^[a-z0-9_-]+(?= *= *\{)" .cargo/config.toml | sort -u)
  PATCHED_COUNT=$(echo "$PATCHED_CRATES" | wc -w)

  if [ "$PATCHED_COUNT" -eq "$MANIFEST_COUNT" ]; then
    check "link patches all $MANIFEST_COUNT manifest crates" 0
  else
    check "link patches all $MANIFEST_COUNT manifest crates" 1 \
      "patched $PATCHED_COUNT: $PATCHED_CRATES vs manifest $MANIFEST_COUNT: $MANIFEST_CRATES"
  fi

  # Verify each manifest crate is patched
  for crate in $MANIFEST_CRATES; do
    if echo "$PATCHED_CRATES" | grep -q "^$crate$"; then
      check "link patches $crate" 0
    else
      check "link patches $crate" 1
    fi
  done
else
  check "link creates config.toml" 1 ".cargo/config.toml not found"
fi

# Clean up
bash "$COMMONS_SH" unlink >/dev/null 2>&1

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
