#!/usr/bin/env bash
#
# Cross-check that cv.tex carries the same canonical contact details as the
# website's single source of truth (jscarrott/src/content.rs).
#
# content.rs is authoritative: this script reads the email/phone literals from
# it and fails if they are missing from cv.tex, so the two can't drift apart.
# Run from anywhere; it resolves paths relative to the repo root.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CONTENT="$ROOT/jscarrott/src/content.rs"
CV="$ROOT/cv.tex"

# Print the string-literal value of a `field: "..."` line from content.rs.
extract() {
  sed -n "s/.*$1: \"\([^\"]*\)\".*/\1/p" "$CONTENT" | head -n1
}

status=0
for field in email phone; do
  value="$(extract "$field")"
  if [[ -z "$value" ]]; then
    echo "✗ could not read '$field' from $CONTENT"
    status=1
    continue
  fi
  if grep -qF -- "$value" "$CV"; then
    echo "✓ cv.tex contains $field: $value"
  else
    echo "✗ cv.tex is out of sync — missing $field: $value (update cv.tex or content.rs)"
    status=1
  fi
done

exit $status
