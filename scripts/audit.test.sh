#!/usr/bin/env bash
# Proves audit.sh goes red on a lockfile that pins a crate with a known
# advisory and green on one that pins none. The red case is checked for exit
# status 1 exactly: cargo-audit uses other codes for a broken lockfile or a
# failed database fetch, and those must not pass for a finding.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUDIT="$HERE/audit.sh"
command -v cargo-audit >/dev/null || { echo "FAIL: cargo-audit not on PATH"; exit 2; }

TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT

# lockfile NAME VERSION -> path to a one-package Cargo.lock
lockfile() {
    local dir="$TMP/$1-$2"
    mkdir -p "$dir"
    printf 'version = 4\n\n[[package]]\nname = "%s"\nversion = "%s"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "%064d"\n' \
        "$1" "$2" 0 > "$dir/Cargo.lock"
    echo "$dir/Cargo.lock"
}

# time < 0.2.23 is RUSTSEC-2020-0071. A lockfile that pins it must go red.
bad="$(lockfile time 0.1.43)"
"$AUDIT" "$bad" >/dev/null 2>&1; rc=$?
if [ "$rc" -ne 1 ]; then echo "FAIL: advisory in lockfile exited $rc, expected 1"; exit 1; fi

good="$(lockfile serde 1.0.228)"
"$AUDIT" "$good" >/dev/null 2>&1; rc=$?
if [ "$rc" -ne 0 ]; then echo "FAIL: clean lockfile exited $rc, expected 0"; exit 1; fi

echo "audit self-test: ok"
