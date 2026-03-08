#!/usr/bin/env bash
#
# compare-screenshots.sh - Run docs screenshot tests twice and compare outputs.
#
# Usage:
#   ./e2e/scripts/compare-screenshots.sh [SPEC_FILE]
#
# Examples:
#   ./e2e/scripts/compare-screenshots.sh                    # all docs tests
#   ./e2e/scripts/compare-screenshots.sh docs/csp.spec.ts   # single file
#
# Prerequisites:
#   - e2e stack running (make e2e-up)
#   - ImageMagick installed (brew install imagemagick)
#
# Outputs to /tmp/screenshot-diff/:
#   run-a/          - screenshots from first run
#   run-b/          - screenshots from second run
#   diffs/          - visual diff images (red = changed pixels)
#   report.txt      - summary with per-file pixel diff counts and byte sizes

set -euo pipefail

SPEC="${1:-}"
OUT_DIR="/tmp/screenshot-diff"
DOCS_IMG="components/secutils-docs/static/img/docs"
RUN_A="$OUT_DIR/run-a"
RUN_B="$OUT_DIR/run-b"
DIFF_DIR="$OUT_DIR/diffs"

rm -rf "$OUT_DIR"
mkdir -p "$RUN_A" "$RUN_B" "$DIFF_DIR"

run_screenshots() {
  local args=""
  if [ -n "$SPEC" ]; then
    args="$SPEC"
  fi
  cd e2e && npx playwright test --config playwright.docs.config.ts $args 2>&1
  cd ..
}

snapshot_images() {
  local dest="$1"
  find "$DOCS_IMG" -name '*.png' -print0 | while IFS= read -r -d '' f; do
    rel="${f#$DOCS_IMG/}"
    mkdir -p "$dest/$(dirname "$rel")"
    cp "$f" "$dest/$rel"
  done
}

echo "=== Run A ==="
run_screenshots > "$OUT_DIR/run-a.log" 2>&1 || true
snapshot_images "$RUN_A"

echo "=== Run B ==="
run_screenshots > "$OUT_DIR/run-b.log" 2>&1 || true
snapshot_images "$RUN_B"

echo "=== Comparing ==="
{
  echo "Screenshot Comparison Report"
  echo "============================"
  echo ""
  echo "Date: $(date)"
  echo ""

  changed=0
  identical=0
  total=0

  find "$RUN_A" -name '*.png' -print0 | sort -z | while IFS= read -r -d '' a_file; do
    rel="${a_file#$RUN_A/}"
    b_file="$RUN_B/$rel"
    total=$((total + 1))

    if [ ! -f "$b_file" ]; then
      echo "MISSING in run-b: $rel"
      changed=$((changed + 1))
      continue
    fi

    a_size=$(stat -f%z "$a_file" 2>/dev/null || stat -c%s "$a_file" 2>/dev/null)
    b_size=$(stat -f%z "$b_file" 2>/dev/null || stat -c%s "$b_file" 2>/dev/null)

    diff_file="$DIFF_DIR/$rel"
    mkdir -p "$(dirname "$diff_file")"

    # Pixel-level comparison via ImageMagick
    pixel_diff=$(compare -metric AE "$a_file" "$b_file" "$diff_file" 2>&1 || true)

    if [ "$pixel_diff" = "0" ] && [ "$a_size" = "$b_size" ]; then
      echo "IDENTICAL: $rel (${a_size} bytes)"
      identical=$((identical + 1))
      rm -f "$diff_file"
    elif [ "$pixel_diff" = "0" ]; then
      echo "PIXEL-SAME BUT BYTE-DIFF: $rel (A=${a_size} B=${b_size}, diff=${pixel_diff}px)"
      changed=$((changed + 1))
    else
      echo "CHANGED: $rel (A=${a_size} B=${b_size}, diff=${pixel_diff}px)"
      changed=$((changed + 1))
    fi
  done

  echo ""
  echo "Summary: $identical identical, $changed changed out of $total total"
} | tee "$OUT_DIR/report.txt"

echo ""
echo "Results in: $OUT_DIR"
echo "  run-a.log / run-b.log  - Playwright output"
echo "  run-a/ / run-b/        - PNG snapshots"
echo "  diffs/                 - visual diff images (red pixels = differences)"
echo "  report.txt             - this report"
