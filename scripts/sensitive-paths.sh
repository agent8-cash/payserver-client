#!/usr/bin/env bash
# Reads changed paths on stdin and prints the ones in a sensitive area: auth and
# crypto, wallet and receiving-key handling, API keys and secrets, env files,
# and the security gate itself. Prints nothing when no path qualifies. Always
# exits 0; the caller decides what a hit means. A hit is a review request, not
# a failure.
set -uo pipefail

PATTERN='(^|/)(auth|crypto)[^/]*(/|\.rs$)'
PATTERN+='|(^|/)(wallets?|receiving[_-]?key|api[_-]?keys?|keys?|secrets?|seeds?)([/_.-]|$)'
PATTERN+='|(^|/)\.env'
PATTERN+='|^\.github/'
PATTERN+='|^scripts/(secret-scan|sensitive-paths|audit)'

grep -E "$PATTERN" || true
