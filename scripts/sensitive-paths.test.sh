#!/usr/bin/env bash
# Proves sensitive-paths.sh flags the areas a human must review and leaves
# ordinary paths alone. A flag that matches nothing passes every run, so the
# positive cases are the point.
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FLAG="$HERE/sensitive-paths.sh"
fail=0

check() { # expected(hit|none) path
    local got
    got="$(printf '%s\n' "$2" | "$FLAG")"
    if [ "$1" = hit ] && [ -z "$got" ]; then echo "FAIL: not flagged: $2"; fail=1; fi
    if [ "$1" = none ] && [ -n "$got" ]; then echo "FAIL: wrongly flagged: $2"; fail=1; fi
}

for p in src/auth_crypto.rs \
         src/pages/wallets.rs \
         src/util/receiving_key.rs \
         src/pages/settings/api_keys.rs \
         src/secrets/store.rs \
         .env.production \
         .github/workflows/ci.yml \
         scripts/secret-scan.sh \
         scripts/sensitive-paths.sh; do
    check hit "$p"
done

for p in README.md \
         scripts/check.sh \
         src/util/keyboard.rs \
         src/pages/payments/list.rs \
         src/pages/invoices/detail.rs \
         styles.css; do
    check none "$p"
done

[ "$fail" -eq 0 ] && echo "sensitive-paths self-test: ok"
exit "$fail"
