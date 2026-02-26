#!/usr/bin/env python3
"""Convert Ghostty theme (key=value) to ori_term TOML theme format.

Usage:
    python3 tools/convert-ghostty.py <input_theme> [output.toml]

If output is omitted, prints to stdout.
"""

import sys
from pathlib import Path


def normalize_hex(value: str) -> str:
    """Normalize a hex color to #rrggbb format."""
    value = value.strip().lstrip("#")
    if value.startswith("0x") or value.startswith("0X"):
        value = value[2:]
    if len(value) != 6:
        print(f"Warning: unexpected color format '{value}'", file=sys.stderr)
        return f"#{value}"
    return f"#{value.lower()}"


def convert(input_path: str) -> str:
    """Convert a Ghostty theme file to TOML string."""
    palette = {}
    fg = None
    bg = None
    cursor = None
    sel_fg = None
    sel_bg = None

    with open(input_path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue

            if "=" not in line:
                continue

            key, _, value = line.partition("=")
            key = key.strip()
            value = value.strip()

            if key == "palette":
                # Format: palette = N=#RRGGBB
                idx_str, _, color = value.partition("=")
                try:
                    idx = int(idx_str.strip())
                    if 0 <= idx <= 15:
                        palette[idx] = normalize_hex(color)
                except ValueError:
                    print(f"Warning: invalid palette index '{idx_str}'", file=sys.stderr)
            elif key == "foreground":
                fg = normalize_hex(value)
            elif key == "background":
                bg = normalize_hex(value)
            elif key == "cursor-color":
                cursor = normalize_hex(value)
            elif key == "selection-foreground":
                sel_fg = normalize_hex(value)
            elif key == "selection-background":
                sel_bg = normalize_hex(value)

    # Build ANSI array, filling missing entries with black.
    ansi = []
    for i in range(16):
        ansi.append(palette.get(i, "#000000"))

    if not fg:
        fg = ansi[7]  # Default foreground = white/light gray.
    if not bg:
        bg = ansi[0]  # Default background = black.
    if not cursor:
        cursor = fg

    name = Path(input_path).stem

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

    if sel_fg:
        lines.append(f'selection_foreground = "{sel_fg}"')
    if sel_bg:
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
