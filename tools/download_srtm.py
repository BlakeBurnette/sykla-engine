#!/usr/bin/env python3
"""Download SRTM1 .hgt tiles from NASA/USGS public archives.

Usage:
    python tools/download_srtm.py N35W079

Downloads the tile to assets/srtm/ (creating the directory if needed).
Requires an internet connection. Uses the USGS SRTM archive (no auth required).
"""
import argparse
import sys
import urllib.request
import zipfile
import io
from pathlib import Path

# USGS mirror — no authentication required
SRTM_BASE_URL = "https://elevation-tiles-prod.s3.amazonaws.com/skadi"


def download_tile(tile_name: str, output_dir: Path) -> Path:
    """Download an SRTM1 .hgt tile and return the path."""
    output_dir.mkdir(parents=True, exist_ok=True)
    hgt_path = output_dir / f"{tile_name}.hgt"

    if hgt_path.exists():
        print(f"Already cached: {hgt_path}")
        return hgt_path

    # Skadi format: /N35/N35W079.hgt.gz
    lat_prefix = tile_name[:3]  # e.g., "N35"
    url = f"{SRTM_BASE_URL}/{lat_prefix}/{tile_name}.hgt.gz"

    print(f"Downloading: {url}")
    try:
        import gzip

        req = urllib.request.Request(url, headers={"User-Agent": "sykla-engine/1.0"})
        with urllib.request.urlopen(req, timeout=60) as response:
            compressed = response.read()
            data = gzip.decompress(compressed)
            hgt_path.write_bytes(data)
            print(f"Saved: {hgt_path} ({len(data)} bytes)")
            return hgt_path
    except urllib.error.HTTPError as e:
        print(f"HTTP error {e.code}: {e.reason}", file=sys.stderr)
        print(f"URL: {url}", file=sys.stderr)
        sys.exit(1)
    except Exception as e:
        print(f"Download failed: {e}", file=sys.stderr)
        sys.exit(1)


def main():
    parser = argparse.ArgumentParser(description="Download SRTM1 .hgt tiles")
    parser.add_argument(
        "tile", help="Tile name, e.g. N35W079 (latitude/longitude of SW corner)"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("assets/srtm"),
        help="Directory to save tiles (default: assets/srtm/)",
    )
    args = parser.parse_args()

    tile_name = args.tile.upper()
    # Validate format
    if len(tile_name) != 7 or tile_name[0] not in "NS" or tile_name[3] not in "EW":
        parser.error(
            f"Invalid tile name '{tile_name}'. Expected format: N35W079 or S12E034"
        )

    download_tile(tile_name, args.output_dir)


if __name__ == "__main__":
    main()
