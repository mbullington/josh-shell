#!/usr/bin/env bash
# Run every share/*.josh selftest headless, then verify regex.josh's golden
# output has not drifted. Exits nonzero on the first failure.
set -euo pipefail
cd "$(dirname "$0")/.."

JOSH="${JOSH:-target/debug/josh}"
if [ ! -x "$JOSH" ]; then
    echo "check-share: $JOSH not found; run \`cargo build\` first (or set JOSH=/path/to/josh)" >&2
    exit 2
fi

# Dependency order matters: selftests may rely on earlier libraries.
libs="assert regex"
for lib in $libs; do
    echo "selftest $lib"
    "$JOSH" --no-config -c "source share/assert.josh; source share/$lib.josh; ${lib}_selftest(); echo \"ok $lib\""
done

gold_tmp="$(mktemp)"
trap 'rm -f "$gold_tmp"' EXIT
"$JOSH" --no-config -c 'source share/assert.josh; source share/regex.josh; regex_golden()' >"$gold_tmp"
if ! diff -u share/regex.golden.txt "$gold_tmp"; then
    echo "check-share: regex golden output drifted — if intentional, regenerate with:" >&2
    echo "  $JOSH --no-config -c 'source share/assert.josh; source share/regex.josh; regex_golden()' > share/regex.golden.txt" >&2
    exit 1
fi
echo "golden output matches share/regex.golden.txt"
echo "check-share: ok"
