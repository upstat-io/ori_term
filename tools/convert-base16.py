#!/usr/bin/env python3
"""Convert base16 YAML scheme to ori_term TOML theme format.

Usage:
    python3 tools/convert-base16.py <input.yaml> [output.toml]

If output is omitted, prints to stdout.

Base16 color mapping:
    base00-base07: Grayscale shades (00=darkest, 07=lightest)
    base08-base0F: Semantic colors (red, orange, yellow, green, cyan, blue, magenta, brown)

ANSI mapping follows the standard base16 terminal mapping:
    0=base00 (black), 1=base08 (red), 2=base0B (green), 3=base0A (yellow),
    4=base0D (blue), 5=base0E (magenta), 6=base0C (cyan), 7=base05 (white),
    8=base03 (bright black), 9=base08, 10=base0B, 11=base0A,
    12=base0D, 13=base0E, 14=base0C, 15=base07 (bright white)
"""

import sys
from pathlib import Path

import yaml


# Standard base16 → ANSI index mapping.
BASE16_TO_ANSI = [
    "base00",  # 0: Black
    "base08",  # 1: Red
    "base0B",  # 2: Green
    "base0A",  # 3: Yellow
    "base0D",  # 4: Blue
    "base0E",  # 5: Magenta
    "base0C",  # 6: Cyan
    "base05",  # 7: White
    "base03",  # 8: Bright black
    "base08",  # 9: Bright red
    "base0B",  # 10: Bright green
    "base0A",  # 11: Bright yellow
    "base0D",  # 12: Bright blue
    "base0E",  # 13: Bright magenta
    "base0C",  # 14: Bright cyan
    "base07",  # 15: Bright white
]


def normalize_hex(value: str) -> str:
    """Normalize a hex color to #rrggbb format."""
    value = str(value).strip().strip('"').strip("'").lstrip("#")
    return f"#{value.lower()}"


def convert(input_path: str) -> str:
    """Convert a base16 YAML file to TOML string."""
    with open(input_path) as f:
        data = yaml.safe_load(f)

    # Build ANSI array from base16 mapping.
    ansi = []
    for base_key in BASE16_TO_ANSI:
        color = data.get(base_key, data.get(base_key.upper(), "000000"))
        ansi.append(normalize_hex(color))

    fg = normalize_hex(data.get("base05", "cccccc"))
    bg = normalize_hex(data.get("base00", "000000"))
    cursor = normalize_hex(data.get("base0F", data.get("base0D", "ffffff")))

    # Selection: use base02/base05 as sensible defaults.
    sel_bg = normalize_hex(data.get("base02", "333333"))

    name = data.get("scheme", data.get("name", Path(input_path).stem))

    lines = [f'name = "{name}"', ""]
    lines.append("ansi = [")
    for i in range(0, 16, 4):
        chunk = ", ".join(f'"{c}"' for c in ansi[i : i + 4])
        lines.append(f"    {chunk},")
    lines.append("]")
    lines.append("")
    lines.append(f'foreground = "{fg}"')
    lines.append(f'background = "{bg}"')
    lines.append(f'cursor = "{cursor}"')
    lines.append(f'selection_background = "{sel_bg}"')

    return "\n".join(lines) + "\n"


def main():
    if len(sys.argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        sys.exit(1)

    result = convert(sys.argv[1])

    if len(sys.argv) >= 3:
        Path(sys.argv[2]).write_text(result)
        print(f"Wrote {sys.argv[2]}", file=sys.stderr)
    else:
        print(result, end="")


if __name__ == "__main__":
    main()
