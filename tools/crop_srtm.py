#!/usr/bin/env python3
"""Crop an SRTM1 .hgt tile to a bounding box and write a compact .dem file.

Usage:
    python tools/crop_srtm.py <input.hgt> <output.dem> \
        --lat-min 45.33 --lat-max 45.48 --lon-min 6.53 --lon-max 6.69

The .dem format (little-endian):
    f64 lat_min, f64 lon_min, f64 cell_size, u32 rows, u32 cols
    i16[rows*cols] row-major, north-to-south

Presets:
    python tools/crop_srtm.py N45E006.hgt assets/routes/col-de-la-loze.dem --preset col-de-la-loze
"""
import argparse
import struct
import sys
from pathlib import Path

# SRTM1: 1 arc-second, 3601x3601 grid
SRTM1_SIZE = 3601
CELL_SIZE = 1.0 / 3600.0  # ~30m at equator

PRESETS = {
    "col-de-la-loze": {
        "lat_min": 45.33,
        "lat_max": 45.48,
        "lon_min": 6.53,
        "lon_max": 6.69,
    },
    "att": {
        "lat_min": 35.72,
        "lat_max": 36.00,
        "lon_min": -79.00,
        "lon_max": -78.80,
    },
    "black-creek": {
        "lat_min": 35.74,
        "lat_max": 35.83,
        "lon_min": -78.83,
        "lon_max": -78.74,
    },
    "oak-creek": {
        "lat_min": 35.75,
        "lat_max": 35.81,
        "lon_min": -78.83,
        "lon_max": -78.75,
    },
}


def read_hgt(path: Path) -> tuple:
    """Read an SRTM1 .hgt file. Returns (lat_origin, lon_origin, data[3601][3601])."""
    name = path.stem.upper()
    # Parse tile corner from filename: N45E006
    lat_sign = 1 if name[0] == "N" else -1
    lon_sign = 1 if name[3] == "E" else -1
    lat_origin = lat_sign * int(name[1:3])
    lon_origin = lon_sign * int(name[4:7])

    raw = path.read_bytes()
    expected = SRTM1_SIZE * SRTM1_SIZE * 2
    if len(raw) != expected:
        print(f"Error: expected {expected} bytes, got {len(raw)}", file=sys.stderr)
        sys.exit(1)

    # SRTM is big-endian i16
    data = struct.unpack(f">{SRTM1_SIZE * SRTM1_SIZE}h", raw)
    return lat_origin, lon_origin, data


def crop_and_write(
    lat_origin: int,
    lon_origin: int,
    data: tuple,
    lat_min: float,
    lat_max: float,
    lon_min: float,
    lon_max: float,
    output: Path,
):
    """Crop the SRTM tile and write .dem file."""
    # SRTM grid: row 0 = north edge (lat_origin + 1), row 3600 = south edge (lat_origin)
    # col 0 = west edge (lon_origin), col 3600 = east edge (lon_origin + 1)

    lat_north = lat_origin + 1.0  # top of tile

    # Convert bounding box to row/col indices
    row_min = max(0, int((lat_north - lat_max) / CELL_SIZE))
    row_max = min(SRTM1_SIZE - 1, int((lat_north - lat_min) / CELL_SIZE) + 1)
    col_min = max(0, int((lon_min - lon_origin) / CELL_SIZE))
    col_max = min(SRTM1_SIZE - 1, int((lon_max - lon_origin) / CELL_SIZE) + 1)

    rows = row_max - row_min + 1
    cols = col_max - col_min + 1

    # The lat_min of the output is the latitude of the southernmost row
    out_lat_min = lat_north - row_max * CELL_SIZE
    out_lon_min = lon_origin + col_min * CELL_SIZE

    print(f"Crop: rows {row_min}..{row_max} ({rows}), cols {col_min}..{col_max} ({cols})")
    print(f"Output: lat [{out_lat_min:.6f}, {out_lat_min + (rows-1)*CELL_SIZE:.6f}], "
          f"lon [{out_lon_min:.6f}, {out_lon_min + (cols-1)*CELL_SIZE:.6f}]")
    print(f"Size: {rows * cols * 2 + 32} bytes ({rows * cols * 2 / 1024:.1f} KB data)")

    # Write header
    header = struct.pack("<dddII", out_lat_min, out_lon_min, CELL_SIZE, rows, cols)

    # Extract cropped data (convert from big-endian i16 to little-endian i16)
    pixels = bytearray()
    for r in range(row_min, row_max + 1):
        for c in range(col_min, col_max + 1):
            val = data[r * SRTM1_SIZE + c]
            pixels.extend(struct.pack("<h", val))

    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(header + bytes(pixels))
    print(f"Written: {output} ({len(header) + len(pixels)} bytes)")


def main():
    parser = argparse.ArgumentParser(description="Crop SRTM1 .hgt to .dem")
    parser.add_argument("input", type=Path, help="Input .hgt file")
    parser.add_argument("output", type=Path, help="Output .dem file")
    parser.add_argument("--preset", choices=PRESETS.keys(), help="Use a preset bounding box")
    parser.add_argument("--lat-min", type=float)
    parser.add_argument("--lat-max", type=float)
    parser.add_argument("--lon-min", type=float)
    parser.add_argument("--lon-max", type=float)
    args = parser.parse_args()

    if args.preset:
        p = PRESETS[args.preset]
        lat_min, lat_max = p["lat_min"], p["lat_max"]
        lon_min, lon_max = p["lon_min"], p["lon_max"]
    elif all(v is not None for v in [args.lat_min, args.lat_max, args.lon_min, args.lon_max]):
        lat_min, lat_max = args.lat_min, args.lat_max
        lon_min, lon_max = args.lon_min, args.lon_max
    else:
        parser.error("Provide --preset or all of --lat-min --lat-max --lon-min --lon-max")

    lat_origin, lon_origin, data = read_hgt(args.input)
    print(f"Loaded: {args.input.name} (origin: {lat_origin}N {lon_origin}E)")

    crop_and_write(lat_origin, lon_origin, data, lat_min, lat_max, lon_min, lon_max, args.output)


if __name__ == "__main__":
    main()
