#!/usr/bin/env python3
"""
analyze-screenshot-diffs.py — Deep analysis of screenshot differences between two runs.

Usage:
    python3 e2e/scripts/analyze-screenshot-diffs.py [--dir /tmp/screenshot-diff]

Reads run-a/ and run-b/ from the diff directory, produces a detailed report
showing which regions differ and potential causes.
"""

import argparse
import json
import os
import sys
from pathlib import Path

try:
    from PIL import Image, ImageChops, ImageDraw
except ImportError:
    print("Pillow is required: pip3 install Pillow", file=sys.stderr)
    sys.exit(1)


def analyze_pair(a_path: Path, b_path: Path, diff_dir: Path, rel: str):
    """Compare two PNGs and return analysis dict."""
    img_a = Image.open(a_path)
    img_b = Image.open(b_path)

    result = {
        "file": rel,
        "size_a": os.path.getsize(a_path),
        "size_b": os.path.getsize(b_path),
        "dimensions_a": img_a.size,
        "dimensions_b": img_b.size,
        "byte_identical": open(a_path, "rb").read() == open(b_path, "rb").read(),
    }

    if img_a.size != img_b.size:
        result["error"] = "dimension mismatch"
        result["pixel_diff_count"] = -1
        return result

    diff = ImageChops.difference(img_a.convert("RGBA"), img_b.convert("RGBA"))
    bbox = diff.getbbox()

    if bbox is None:
        result["pixel_diff_count"] = 0
        result["diff_bbox"] = None
        return result

    diff_pixels = 0
    width, height = img_a.size
    diff_data = diff.load()
    changed_rows = set()
    changed_cols = set()

    for y in range(bbox[1], bbox[3]):
        for x in range(bbox[0], bbox[2]):
            r, g, b, a = diff_data[x, y]
            if r > 0 or g > 0 or b > 0:
                diff_pixels += 1
                changed_rows.add(y)
                changed_cols.add(x)

    result["pixel_diff_count"] = diff_pixels
    result["diff_bbox"] = list(bbox)
    result["diff_bbox_size"] = [bbox[2] - bbox[0], bbox[3] - bbox[1]]
    result["diff_area_pct"] = round(
        100 * diff_pixels / (width * height), 4
    )

    # Classify the diff region
    bw = bbox[2] - bbox[0]
    bh = bbox[3] - bbox[1]

    categories = []
    if bh <= 3 and bw > width * 0.5:
        categories.append("horizontal-line (likely scrollbar or separator)")
    if bw <= 3 and bh > height * 0.3:
        categories.append("vertical-line (likely cursor or scrollbar)")
    if diff_pixels < 50:
        categories.append("subpixel-rendering")
    if 20 < bh < 40 and bw < 200:
        categories.append("possible-text-change")
    if bh > height * 0.5 and bw > width * 0.5:
        categories.append("major-layout-shift")
    if diff_pixels > 100 and bw < 100 and bh < 100:
        categories.append("small-component-change")

    result["categories"] = categories if categories else ["unclassified"]

    # Save annotated diff image
    out_path = diff_dir / rel
    out_path.parent.mkdir(parents=True, exist_ok=True)

    annotated = img_a.copy().convert("RGBA")
    overlay = Image.new("RGBA", img_a.size, (0, 0, 0, 0))
    draw = ImageDraw.Draw(overlay)

    for y in range(bbox[1], bbox[3]):
        for x in range(bbox[0], bbox[2]):
            r, g, b, a = diff_data[x, y]
            if r > 0 or g > 0 or b > 0:
                draw.point((x, y), fill=(255, 0, 0, 128))

    draw.rectangle(bbox, outline=(255, 0, 0, 255), width=2)
    annotated = Image.alpha_composite(annotated, overlay)
    annotated.save(str(out_path))

    return result


def main():
    parser = argparse.ArgumentParser(description="Analyze screenshot diffs")
    parser.add_argument("--dir", default="/tmp/screenshot-diff", help="Diff directory")
    args = parser.parse_args()

    base = Path(args.dir)
    run_a = base / "run-a"
    run_b = base / "run-b"
    diff_dir = base / "analysis"

    if not run_a.exists() or not run_b.exists():
        print(f"Missing {run_a} or {run_b}. Run compare-screenshots.sh first.")
        sys.exit(1)

    diff_dir.mkdir(parents=True, exist_ok=True)

    results = []
    changed = []
    byte_diff_only = []
    identical = []

    for a_file in sorted(run_a.rglob("*.png")):
        rel = str(a_file.relative_to(run_a))
        b_file = run_b / rel

        if not b_file.exists():
            results.append({"file": rel, "error": "missing in run-b"})
            continue

        r = analyze_pair(a_file, b_file, diff_dir, rel)
        results.append(r)

        if r.get("byte_identical"):
            identical.append(rel)
        elif r.get("pixel_diff_count", 0) == 0:
            byte_diff_only.append(rel)
        elif r.get("pixel_diff_count", 0) > 0:
            changed.append(r)

    # Print report
    print("=" * 80)
    print("SCREENSHOT STABILITY ANALYSIS")
    print("=" * 80)
    print()
    print(f"Total files:       {len(results)}")
    print(f"Byte-identical:    {len(identical)}")
    print(f"Byte-diff only:    {len(byte_diff_only)} (visually same, metadata differs)")
    print(f"Pixel differences: {len(changed)}")
    print()

    if byte_diff_only:
        print("-" * 60)
        print("FILES WITH BYTE-LEVEL DIFFERENCES ONLY (no visible change)")
        print("-" * 60)
        for f in byte_diff_only:
            r = next(x for x in results if x["file"] == f)
            print(f"  {f}")
            print(f"    size A={r['size_a']} B={r['size_b']} delta={r['size_b']-r['size_a']}")
        print()

    if changed:
        print("-" * 60)
        print("FILES WITH PIXEL DIFFERENCES")
        print("-" * 60)
        for r in sorted(changed, key=lambda x: -x["pixel_diff_count"]):
            print(f"  {r['file']}")
            print(f"    pixels: {r['pixel_diff_count']} ({r['diff_area_pct']}% of image)")
            print(f"    bbox: {r['diff_bbox']} ({r['diff_bbox_size'][0]}x{r['diff_bbox_size'][1]})")
            print(f"    size A={r['size_a']} B={r['size_b']}")
            print(f"    categories: {', '.join(r['categories'])}")
            print()

    # Save JSON for programmatic analysis
    report_path = base / "analysis-report.json"
    with open(report_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"Full report: {report_path}")
    print(f"Annotated diffs: {diff_dir}/")


if __name__ == "__main__":
    main()
