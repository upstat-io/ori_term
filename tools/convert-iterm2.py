#!/usr/bin/env python3
"""Convert iTerm2 .itermcolors (XML plist) to ori_term TOML theme format.

Usage:
    python3 tools/convert-iterm2.py <input.itermcolors> [output.toml]

If output is omitted, prints to stdout.
"""

import plistlib
import sys
from pathlib import Path


def component_to_hex(color_dict: dict) -> str:
    """Convert iTerm2 float RGB components to #RRGGBB hex."""
    r = int(round(color_dict["Red Component"] * 255))
    g = int(round(color_dict["Green Component"] * 255))
    b = int(round(color_dict["Blue Component"] * 255))
    return f"#{r:02x}{g:02x}{b:02x}"


def convert(input_path: str) -> str:
    """Convert an .itermcolors file to TOML string."""
    with open(input_path, "rb") as f:
        plist = plistlib.load(f)

    # ANSI colors 0-15.
    ansi = []
    for i in range(16):
        key = f"Ansi {i} Color"
        if key not in plist:
            print(f"Warning: missing '{key}', using black", file=sys.stderr)
            ansi.append("#000000")
        else:
            ansi.append(component_to_hex(plist[key]))

    fg = component_to_hex(plist["Foreground Color"])
    bg = component_to_hex(plist["Background Color"])
    cursor = component_to_hex(plist.get("Cursor Color", plist["Foreground Color"]))

    sel_fg = None
    sel_bg = None
    if "Selected Text Color" in plist:
        sel_fg = component_to_hex(plist["Selected Text Color"])
    if "Selection Color" in plist:
        sel_bg = component_to_hex(plist["Selection Color"])

    # Derive name from filename.
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
