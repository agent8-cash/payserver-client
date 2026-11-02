#!/usr/bin/env bash
# Fails when a lockfile pins a crate with a known advisory. cargo-audit exits 1
# on a finding and other codes on a broken lockfile or failed fetch; the caller
# reads any non-zero as a failed gate.
# Usage: audit.sh [lockfile]   (default: ./Cargo.lock)
set -uo pipefail

exec cargo audit --file "${1:-Cargo.lock}"
