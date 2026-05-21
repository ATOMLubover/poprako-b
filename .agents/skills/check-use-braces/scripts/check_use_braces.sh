#!/usr/bin/env bash
# check_use_braces.sh — Check Rust use-statements for non-leaf braces.
#
# Rule: Do NOT allow a::{b, c::d} style.
# Only allow braces at the final leaf level, e.g. a::b::{c, d}.
#
# Usage:
#   ./scripts/check_use_braces.sh              # check src/ recursively
#   ./scripts/check_use_braces.sh path/to/file  # check specific files

set -euo pipefail

if [ $# -gt 0 ]; then
    files=("$@")
else
    # Bread and butter: find all .rs files under src/, excluding target/
    mapfile -t files < <(find . -path ./target -prune -o -name '*.rs' -print)
fi

violations=0

for f in "${files[@]}"; do
    # Collapse a multi-line use statement onto one line for easier parsing.
    # This uses awk: when we see a line starting with optional whitespace + "use",
    # start accumulating; for every subsequent line, keep appending until we
    # hit a semicolon.  Then run the inline check.
    # shellcheck disable=SC2312
    awk '
    /^\s*use\s/ { buf = $0; next }
    { if (buf != "") buf = buf " " $0 }
    /;/ {
        if (buf != "") {
            # Only care about lines that have braces.
            if (index(buf, "{") > 0 && index(buf, "}") > 0) {
                # Grab everything between the first { and the matching }.
                # We assume no nested braces (unusual in use statements).
                start = index(buf, "{")
                end   = index(buf, "}")
                inner = substr(buf, start + 1, end - start - 1)

                # Split on commas and check every item.
                n = split(inner, items, ",")
                for (i = 1; i <= n; i++) {
                    gsub(/^[[:space:]]+|[[:space:]]+$/, "", items[i])
                    if (items[i] != "" && index(items[i], "::") > 0) {
                        print FILENAME ":" FNR " - VIOLATION"
                        printf "    %s\n", buf
                        exit_code = 1
                        break
                    }
                }
            }
            buf = ""
        }
    }
    ' "$f"
    rc=$?
    if [ "$rc" -ne 0 ]; then
        violations=$((violations + 1))
    fi
done

if [ "$violations" -eq 0 ]; then
    echo "✓ All use statements pass — no non-leaf braces found."
else
    echo "✗ Found $violations file(s) with violations."
fi
exit "$violations"
