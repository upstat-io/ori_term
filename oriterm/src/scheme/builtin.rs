//! Built-in color scheme definitions (50+ schemes).
//!
//! Pure const data — no logic. Each scheme defines 16 ANSI colors plus
//! foreground, background, and cursor colors.

// Hex color literals (0xRRGGBB) intentionally match CSS/HTML color codes.
// Adding underscores (0x00RR_GGBB) would obscure the R/G/B byte boundaries.
#![allow(clippy::unreadable_literal)]

use oriterm_core::Rgb;

use super::BuiltinScheme;

/// Helper to construct `Rgb` from a 24-bit hex value at compile time.
const fn rgb(hex: u32) -> Rgb {
    Rgb {
        r: ((hex >> 16) & 0xFF) as u8,
        g: ((hex >> 8) & 0xFF) as u8,
        b: (hex & 0xFF) as u8,
    }
}

/// Helper to construct a 16-entry ANSI palette from hex values.
const fn ansi16(c: [u32; 16]) -> [Rgb; 16] {
    [
        rgb(c[0]),
        rgb(c[1]),
        rgb(c[2]),
        rgb(c[3]),
        rgb(c[4]),
        rgb(c[5]),
        rgb(c[6]),
        rgb(c[7]),
        rgb(c[8]),
        rgb(c[9]),
        rgb(c[10]),
        rgb(c[11]),
        rgb(c[12]),
        rgb(c[13]),
        rgb(c[14]),
        rgb(c[15]),
    ]
}

// Ported from _old/src/palette/schemes.rs.
const CATPPUCCIN_MOCHA: BuiltinScheme = BuiltinScheme {
    name: "Catppuccin Mocha",
    ansi: ansi16([
        0x45475a, 0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xbac2de, 0x585b70,
        0xf38ba8, 0xa6e3a1, 0xf9e2af, 0x89b4fa, 0xf5c2e7, 0x94e2d5, 0xa6adc8,
    ]),
    fg: rgb(0xcdd6f4),
    bg: rgb(0x1e1e2e),
    cursor: rgb(0xf5e0dc),
};

const CATPPUCCIN_LATTE: BuiltinScheme = BuiltinScheme {
    name: "Catppuccin Latte",
    ansi: ansi16([
        0x5c5f77, 0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179c99, 0xacb0be, 0x6c6f85,
        0xd20f39, 0x40a02b, 0xdf8e1d, 0x1e66f5, 0xea76cb, 0x179c99, 0xbcc0cc,
    ]),
    fg: rgb(0x4c4f69),
    bg: rgb(0xeff1f5),
    cursor: rgb(0xdc8a78),
};

const CATPPUCCIN_FRAPPE: BuiltinScheme = BuiltinScheme {
    name: "Catppuccin Frappe",
    ansi: ansi16([
        0x51576d, 0xe78284, 0xa6d189, 0xe5c890, 0x8caaee, 0xf4b8e4, 0x81c8be, 0xb5bfe2, 0x626880,
        0xe78284, 0xa6d189, 0xe5c890, 0x8caaee, 0xf4b8e4, 0x81c8be, 0xa5adce,
    ]),
    fg: rgb(0xc6d0f5),
    bg: rgb(0x303446),
    cursor: rgb(0xf2d5cf),
};

const CATPPUCCIN_MACCHIATO: BuiltinScheme = BuiltinScheme {
    name: "Catppuccin Macchiato",
    ansi: ansi16([
        0x494d64, 0xed8796, 0xa6da95, 0xeed49f, 0x8aadf4, 0xf5bde6, 0x8bd5ca, 0xb8c0e0, 0x5b6078,
        0xed8796, 0xa6da95, 0xeed49f, 0x8aadf4, 0xf5bde6, 0x8bd5ca, 0xa5adcb,
    ]),
    fg: rgb(0xcad3f5),
    bg: rgb(0x24273a),
    cursor: rgb(0xf4dbd6),
};

// Ported from _old/src/palette/schemes.rs.
const ONE_DARK: BuiltinScheme = BuiltinScheme {
    name: "One Dark",
    ansi: ansi16([
        0x282c34, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xabb2bf, 0x545862,
        0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xbec5d4,
    ]),
    fg: rgb(0xabb2bf),
    bg: rgb(0x282c34),
    cursor: rgb(0x528bff),
};

const ONE_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "One Light",
    ansi: ansi16([
        0x383a42, 0xe45649, 0x50a14f, 0xc18401, 0x4078f2, 0xa626a4, 0x0184bc, 0xa0a1a7, 0x4f525e,
        0xe45649, 0x50a14f, 0xc18401, 0x4078f2, 0xa626a4, 0x0184bc, 0xfafafa,
    ]),
    fg: rgb(0x383a42),
    bg: rgb(0xfafafa),
    cursor: rgb(0x526fff),
};

// Ported from _old/src/palette/schemes.rs.
const SOLARIZED_DARK: BuiltinScheme = BuiltinScheme {
    name: "Solarized Dark",
    ansi: ansi16([
        0x073642, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0xeee8d5, 0x002b36,
        0xcb4b16, 0x586e75, 0x657b83, 0x839496, 0x6c71c4, 0x93a1a1, 0xfdf6e3,
    ]),
    fg: rgb(0x839496),
    bg: rgb(0x002b36),
    cursor: rgb(0x839496),
};

const SOLARIZED_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Solarized Light",
    ansi: ansi16([
        0xeee8d5, 0xdc322f, 0x859900, 0xb58900, 0x268bd2, 0xd33682, 0x2aa198, 0x073642, 0xfdf6e3,
        0xcb4b16, 0x93a1a1, 0x839496, 0x657b83, 0x6c71c4, 0x586e75, 0x002b36,
    ]),
    fg: rgb(0x657b83),
    bg: rgb(0xfdf6e3),
    cursor: rgb(0x657b83),
};

// Ported from _old/src/palette/schemes.rs.
const DRACULA: BuiltinScheme = BuiltinScheme {
    name: "Dracula",
    ansi: ansi16([
        0x21222c, 0xff5555, 0x50fa7b, 0xf1fa8c, 0xbd93f9, 0xff79c6, 0x8be9fd, 0xf8f8f2, 0x6272a4,
        0xff6e6e, 0x69ff94, 0xffffa5, 0xd6acff, 0xff92df, 0xa4ffff, 0xffffff,
    ]),
    fg: rgb(0xf8f8f2),
    bg: rgb(0x282a36),
    cursor: rgb(0xf8f8f2),
};

// Ported from _old/src/palette/schemes.rs.
const TOKYO_NIGHT: BuiltinScheme = BuiltinScheme {
    name: "Tokyo Night",
    ansi: ansi16([
        0x15161e, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6, 0x414868,
        0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xc0caf5,
    ]),
    fg: rgb(0xa9b1d6),
    bg: rgb(0x1a1b26),
    cursor: rgb(0xc0caf5),
};

const TOKYO_NIGHT_STORM: BuiltinScheme = BuiltinScheme {
    name: "Tokyo Night Storm",
    ansi: ansi16([
        0x1d202f, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xa9b1d6, 0x414868,
        0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xbb9af7, 0x7dcfff, 0xc0caf5,
    ]),
    fg: rgb(0xa9b1d6),
    bg: rgb(0x24283b),
    cursor: rgb(0xc0caf5),
};

const TOKYO_NIGHT_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Tokyo Night Light",
    ansi: ansi16([
        0xe9e9ed, 0xf52a65, 0x587539, 0x8c6c3e, 0x2e7de9, 0x9854f1, 0x007197, 0x6172b0, 0xa1a6c5,
        0xf52a65, 0x587539, 0x8c6c3e, 0x2e7de9, 0x9854f1, 0x007197, 0x3760bf,
    ]),
    fg: rgb(0x3760bf),
    bg: rgb(0xd5d6db),
    cursor: rgb(0x3760bf),
};

// Ported from _old/src/palette/schemes.rs.
const WEZTERM_DEFAULT: BuiltinScheme = BuiltinScheme {
    name: "WezTerm Default",
    ansi: ansi16([
        0x000000, 0xcc5555, 0x55cc55, 0xcdcd55, 0x5455cb, 0xcc55cc, 0x7acaca, 0xcccccc, 0x555555,
        0xff5555, 0x55ff55, 0xffff55, 0x5555ff, 0xff55ff, 0x55ffff, 0xffffff,
    ]),
    fg: rgb(0xb2b2b2),
    bg: rgb(0x000000),
    cursor: rgb(0x52ad70),
};

const GRUVBOX_DARK: BuiltinScheme = BuiltinScheme {
    name: "Gruvbox Dark",
    ansi: ansi16([
        0x282828, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0xa89984, 0x928374,
        0xfb4934, 0xb8bb26, 0xfabd2f, 0x83a598, 0xd3869b, 0x8ec07c, 0xebdbb2,
    ]),
    fg: rgb(0xebdbb2),
    bg: rgb(0x282828),
    cursor: rgb(0xebdbb2),
};

const GRUVBOX_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Gruvbox Light",
    ansi: ansi16([
        0xfbf1c7, 0xcc241d, 0x98971a, 0xd79921, 0x458588, 0xb16286, 0x689d6a, 0x7c6f64, 0x928374,
        0x9d0006, 0x79740e, 0xb57614, 0x076678, 0x8f3f71, 0x427b58, 0x3c3836,
    ]),
    fg: rgb(0x3c3836),
    bg: rgb(0xfbf1c7),
    cursor: rgb(0x3c3836),
};

const NORD: BuiltinScheme = BuiltinScheme {
    name: "Nord",
    ansi: ansi16([
        0x3b4252, 0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x88c0d0, 0xe5e9f0, 0x4c566a,
        0xbf616a, 0xa3be8c, 0xebcb8b, 0x81a1c1, 0xb48ead, 0x8fbcbb, 0xeceff4,
    ]),
    fg: rgb(0xd8dee9),
    bg: rgb(0x2e3440),
    cursor: rgb(0xd8dee9),
};

const ROSE_PINE: BuiltinScheme = BuiltinScheme {
    name: "Rose Pine",
    ansi: ansi16([
        0x26233a, 0xeb6f92, 0x31748f, 0xf6c177, 0x9ccfd8, 0xc4a7e7, 0xebbcba, 0xe0def4, 0x6e6a86,
        0xeb6f92, 0x31748f, 0xf6c177, 0x9ccfd8, 0xc4a7e7, 0xebbcba, 0xe0def4,
    ]),
    fg: rgb(0xe0def4),
    bg: rgb(0x191724),
    cursor: rgb(0xe0def4),
};

const ROSE_PINE_MOON: BuiltinScheme = BuiltinScheme {
    name: "Rose Pine Moon",
    ansi: ansi16([
        0x393552, 0xeb6f92, 0x3e8fb0, 0xf6c177, 0x9ccfd8, 0xc4a7e7, 0xea9a97, 0xe0def4, 0x6e6a86,
        0xeb6f92, 0x3e8fb0, 0xf6c177, 0x9ccfd8, 0xc4a7e7, 0xea9a97, 0xe0def4,
    ]),
    fg: rgb(0xe0def4),
    bg: rgb(0x232136),
    cursor: rgb(0xe0def4),
};

const ROSE_PINE_DAWN: BuiltinScheme = BuiltinScheme {
    name: "Rose Pine Dawn",
    ansi: ansi16([
        0xf2e9e1, 0xb4637a, 0x286983, 0xea9d34, 0x56949f, 0x907aa9, 0xd7827e, 0x575279, 0x9893a5,
        0xb4637a, 0x286983, 0xea9d34, 0x56949f, 0x907aa9, 0xd7827e, 0x575279,
    ]),
    fg: rgb(0x575279),
    bg: rgb(0xfaf4ed),
    cursor: rgb(0x575279),
};

const EVERFOREST_DARK: BuiltinScheme = BuiltinScheme {
    name: "Everforest Dark",
    ansi: ansi16([
        0x475258, 0xe67e80, 0xa7c080, 0xdbbc7f, 0x7fbbb3, 0xd699b6, 0x83c092, 0xd3c6aa, 0x7a8478,
        0xe67e80, 0xa7c080, 0xdbbc7f, 0x7fbbb3, 0xd699b6, 0x83c092, 0xd3c6aa,
    ]),
    fg: rgb(0xd3c6aa),
    bg: rgb(0x2d353b),
    cursor: rgb(0xd3c6aa),
};

const EVERFOREST_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Everforest Light",
    ansi: ansi16([
        0xf3ead3, 0xf85552, 0x8da101, 0xdfa000, 0x3a94c5, 0xdf69ba, 0x35a77c, 0x5c6a72, 0x939f91,
        0xf85552, 0x8da101, 0xdfa000, 0x3a94c5, 0xdf69ba, 0x35a77c, 0x5c6a72,
    ]),
    fg: rgb(0x5c6a72),
    bg: rgb(0xfdf6e3),
    cursor: rgb(0x5c6a72),
};

const KANAGAWA: BuiltinScheme = BuiltinScheme {
    name: "Kanagawa",
    ansi: ansi16([
        0x16161d, 0xc34043, 0x76946a, 0xc0a36e, 0x7e9cd8, 0x957fb8, 0x6a9589, 0xc8c093, 0x727169,
        0xe82424, 0x98bb6c, 0xe6c384, 0x7fb4ca, 0x938aa9, 0x7aa89f, 0xdcd7ba,
    ]),
    fg: rgb(0xdcd7ba),
    bg: rgb(0x1f1f28),
    cursor: rgb(0xc8c093),
};

const KANAGAWA_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Kanagawa Light",
    ansi: ansi16([
        0xc7c7c7, 0xc84053, 0x6f894e, 0x77713f, 0x4d699b, 0xb35b79, 0x597b75, 0x545464, 0xa6a69c,
        0xe82424, 0x6f894e, 0x77713f, 0x4d699b, 0xb35b79, 0x597b75, 0x1f1f28,
    ]),
    fg: rgb(0x1f1f28),
    bg: rgb(0xf2ecbc),
    cursor: rgb(0x43436c),
};

const AYU_DARK: BuiltinScheme = BuiltinScheme {
    name: "Ayu Dark",
    ansi: ansi16([
        0x01060e, 0xea6c73, 0x91b362, 0xf9af4f, 0x53bdfa, 0xfae994, 0x90e1c6, 0xc7c7c7, 0x686868,
        0xf07178, 0xc2d94c, 0xffb454, 0x59c2ff, 0xffee99, 0x95e6cb, 0xffffff,
    ]),
    fg: rgb(0xbfbdb6),
    bg: rgb(0x0d1017),
    cursor: rgb(0xe6b450),
};

const AYU_MIRAGE: BuiltinScheme = BuiltinScheme {
    name: "Ayu Mirage",
    ansi: ansi16([
        0x191e2a, 0xed8274, 0xa6cc70, 0xfad07b, 0x6dcbfa, 0xcfbafa, 0x90e1c6, 0xc7c7c7, 0x686868,
        0xf28779, 0xbae67e, 0xffd580, 0x73d0ff, 0xd4bfff, 0x95e6cb, 0xffffff,
    ]),
    fg: rgb(0xcccac2),
    bg: rgb(0x1f2430),
    cursor: rgb(0xffcc66),
};

const AYU_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Ayu Light",
    ansi: ansi16([
        0x000000, 0xff3333, 0x86b300, 0xf29718, 0x41a6d9, 0xf07178, 0x4dbf99, 0xc7c7c7, 0x686868,
        0xe65050, 0x99cc00, 0xe6b673, 0x55b4d4, 0xf27983, 0x5ccfab, 0xffffff,
    ]),
    fg: rgb(0x5c6166),
    bg: rgb(0xfafafa),
    cursor: rgb(0xff6a00),
};

const MATERIAL_DARK: BuiltinScheme = BuiltinScheme {
    name: "Material Dark",
    ansi: ansi16([
        0x546e7a, 0xff5370, 0xc3e88d, 0xffcb6b, 0x82aaff, 0xc792ea, 0x89ddff, 0xeeffff, 0x546e7a,
        0xff5370, 0xc3e88d, 0xffcb6b, 0x82aaff, 0xc792ea, 0x89ddff, 0xeeffff,
    ]),
    fg: rgb(0xeeffff),
    bg: rgb(0x263238),
    cursor: rgb(0xffcc00),
};

const MATERIAL_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Material Light",
    ansi: ansi16([
        0x546e7a, 0xff5370, 0x91b859, 0xffb62c, 0x6182b8, 0x7c4dff, 0x39adb5, 0x80cbc4, 0x546e7a,
        0xff5370, 0x91b859, 0xffb62c, 0x6182b8, 0x7c4dff, 0x39adb5, 0x80cbc4,
    ]),
    fg: rgb(0x80cbc4),
    bg: rgb(0xfafafa),
    cursor: rgb(0x272727),
};

const MONOKAI: BuiltinScheme = BuiltinScheme {
    name: "Monokai",
    ansi: ansi16([
        0x272822, 0xf92672, 0xa6e22e, 0xf4bf75, 0x66d9ef, 0xae81ff, 0xa1efe4, 0xf8f8f2, 0x75715e,
        0xf92672, 0xa6e22e, 0xf4bf75, 0x66d9ef, 0xae81ff, 0xa1efe4, 0xf9f8f5,
    ]),
    fg: rgb(0xf8f8f2),
    bg: rgb(0x272822),
    cursor: rgb(0xf8f8f0),
};

const NIGHTFOX: BuiltinScheme = BuiltinScheme {
    name: "Nightfox",
    ansi: ansi16([
        0x393b44, 0xc94f6d, 0x81b29a, 0xdbc074, 0x719cd6, 0x9d79d6, 0x63cdcf, 0xdfdfe0, 0x575860,
        0xd16983, 0x8ebaa4, 0xe0c989, 0x86abdc, 0xbaa1e2, 0x7ad5d6, 0xe4e4e5,
    ]),
    fg: rgb(0xcdcecf),
    bg: rgb(0x192330),
    cursor: rgb(0xcdcecf),
};

const DAWNFOX: BuiltinScheme = BuiltinScheme {
    name: "Dawnfox",
    ansi: ansi16([
        0x575279, 0xb4637a, 0x618774, 0xea9d34, 0x286983, 0x907aa9, 0x56949f, 0xe5e9f0, 0x5b5078,
        0xc26d85, 0x629f81, 0xeea846, 0x2d81a3, 0x9b84b2, 0x5fa7b1, 0xeef0f3,
    ]),
    fg: rgb(0x575279),
    bg: rgb(0xfaf4ed),
    cursor: rgb(0x575279),
};

const CARBONFOX: BuiltinScheme = BuiltinScheme {
    name: "Carbonfox",
    ansi: ansi16([
        0x282828, 0xee5396, 0x25be6a, 0x08bdba, 0x78a9ff, 0xbe95ff, 0x33b1ff, 0xdfdfe0, 0x484848,
        0xf16da6, 0x46c880, 0x2dc7c4, 0x8cb6ff, 0xc8a5ff, 0x52bdff, 0xe4e4e5,
    ]),
    fg: rgb(0xf2f4f8),
    bg: rgb(0x161616),
    cursor: rgb(0xf2f4f8),
};

const GITHUB_DARK: BuiltinScheme = BuiltinScheme {
    name: "GitHub Dark",
    ansi: ansi16([
        0x484f58, 0xff7b72, 0x7ee787, 0xd29922, 0x79c0ff, 0xd2a8ff, 0xa5d6ff, 0xb1bac4, 0x6e7681,
        0xffa198, 0x56d364, 0xe3b341, 0xa5d6ff, 0xd2a8ff, 0xb6e3ff, 0xf0f6fc,
    ]),
    fg: rgb(0xc9d1d9),
    bg: rgb(0x0d1117),
    cursor: rgb(0xc9d1d9),
};

const GITHUB_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "GitHub Light",
    ansi: ansi16([
        0x24292e, 0xd73a49, 0x22863a, 0xb08800, 0x0366d6, 0x6f42c1, 0x1b7c83, 0x6a737d, 0x959da5,
        0xcb2431, 0x28a745, 0xdbab09, 0x2188ff, 0x8a63d2, 0x3192aa, 0x24292e,
    ]),
    fg: rgb(0x24292e),
    bg: rgb(0xffffff),
    cursor: rgb(0x24292e),
};

const GITHUB_DARK_DIMMED: BuiltinScheme = BuiltinScheme {
    name: "GitHub Dark Dimmed",
    ansi: ansi16([
        0x545d68, 0xf47067, 0x57ab5a, 0xc69026, 0x539bf5, 0xb083f0, 0x76e3ea, 0xadbac7, 0x636e7b,
        0xff938a, 0x6bc46d, 0xdaaa3f, 0x6cb6ff, 0xdcbdfb, 0xb3f0ff, 0xf0f6fc,
    ]),
    fg: rgb(0xadbac7),
    bg: rgb(0x22272e),
    cursor: rgb(0xadbac7),
};

const SNAZZY: BuiltinScheme = BuiltinScheme {
    name: "Snazzy",
    ansi: ansi16([
        0x282a36, 0xff5c57, 0x5af78e, 0xf3f99d, 0x57c7ff, 0xff6ac1, 0x9aedfe, 0xf1f1f0, 0x686868,
        0xff5c57, 0x5af78e, 0xf3f99d, 0x57c7ff, 0xff6ac1, 0x9aedfe, 0xf1f1f0,
    ]),
    fg: rgb(0xeff0eb),
    bg: rgb(0x282a36),
    cursor: rgb(0x97979b),
};

const TOMORROW_NIGHT: BuiltinScheme = BuiltinScheme {
    name: "Tomorrow Night",
    ansi: ansi16([
        0x1d1f21, 0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x969896,
        0xcc6666, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xffffff,
    ]),
    fg: rgb(0xc5c8c6),
    bg: rgb(0x1d1f21),
    cursor: rgb(0xc5c8c6),
};

const TOMORROW_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Tomorrow Light",
    ansi: ansi16([
        0x000000, 0xc82829, 0x718c00, 0xeab700, 0x4271ae, 0x8959a8, 0x3e999f, 0xffffff, 0x8e908c,
        0xc82829, 0x718c00, 0xeab700, 0x4271ae, 0x8959a8, 0x3e999f, 0xffffff,
    ]),
    fg: rgb(0x4d4d4c),
    bg: rgb(0xffffff),
    cursor: rgb(0x4d4d4c),
};

const ZENBURN: BuiltinScheme = BuiltinScheme {
    name: "Zenburn",
    ansi: ansi16([
        0x4d4d4d, 0x705050, 0x60b48a, 0xdfaf8f, 0x506070, 0xdc8cc3, 0x8cd0d3, 0xdcdccc, 0x709080,
        0xdca3a3, 0xc3bf9f, 0xf0dfaf, 0x94bff3, 0xec93d3, 0x93e0e3, 0xffffff,
    ]),
    fg: rgb(0xdcdccc),
    bg: rgb(0x3f3f3f),
    cursor: rgb(0x73635a),
};

const ICEBERG_DARK: BuiltinScheme = BuiltinScheme {
    name: "Iceberg Dark",
    ansi: ansi16([
        0x1e2132, 0xe27878, 0xb4be82, 0xe2a478, 0x84a0c6, 0xa093c7, 0x89b8c2, 0xc6c8d1, 0x6b7089,
        0xe98989, 0xc0ca8e, 0xe9b189, 0x91acd1, 0xada0d3, 0x95c4ce, 0xd2d4de,
    ]),
    fg: rgb(0xc6c8d1),
    bg: rgb(0x161821),
    cursor: rgb(0xc6c8d1),
};

const ICEBERG_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "Iceberg Light",
    ansi: ansi16([
        0xdcdfe7, 0xcc517a, 0x668e3d, 0xc57339, 0x2d539e, 0x7759b4, 0x3f83a6, 0x33374c, 0x8389a3,
        0xcc3768, 0x598030, 0xb6662d, 0x22478e, 0x6845ad, 0x327698, 0x262a3f,
    ]),
    fg: rgb(0x33374c),
    bg: rgb(0xe8e9ec),
    cursor: rgb(0x33374c),
};

const NIGHT_OWL: BuiltinScheme = BuiltinScheme {
    name: "Night Owl",
    ansi: ansi16([
        0x011627, 0xef5350, 0x22da6e, 0xaddb67, 0x82aaff, 0xc792ea, 0x21c7a8, 0xd6deeb, 0x637777,
        0xef5350, 0x22da6e, 0xaddb67, 0x82aaff, 0xc792ea, 0x21c7a8, 0xffffff,
    ]),
    fg: rgb(0xd6deeb),
    bg: rgb(0x011627),
    cursor: rgb(0x80a4c2),
};

const PALENIGHT: BuiltinScheme = BuiltinScheme {
    name: "Palenight",
    ansi: ansi16([
        0x292d3e, 0xf07178, 0xc3e88d, 0xffcb6b, 0x82aaff, 0xc792ea, 0x89ddff, 0xa6accd, 0x676e95,
        0xf07178, 0xc3e88d, 0xffcb6b, 0x82aaff, 0xc792ea, 0x89ddff, 0xffffff,
    ]),
    fg: rgb(0xa6accd),
    bg: rgb(0x292d3e),
    cursor: rgb(0xffcc00),
};

const HORIZON: BuiltinScheme = BuiltinScheme {
    name: "Horizon",
    ansi: ansi16([
        0x16161c, 0xe95678, 0x29d398, 0xfab795, 0x26bbd9, 0xee64ac, 0x59e1e3, 0xd5d8da, 0x5b5858,
        0xec6a88, 0x3fdaa4, 0xfbc3a7, 0x3fc4de, 0xf075b5, 0x6be4e6, 0xffffff,
    ]),
    fg: rgb(0xe0e0e0),
    bg: rgb(0x1c1e26),
    cursor: rgb(0xe95678),
};

const POIMANDRES: BuiltinScheme = BuiltinScheme {
    name: "Poimandres",
    ansi: ansi16([
        0x1b1e28, 0xd0679d, 0x5de4c7, 0xfffac2, 0x89ddff, 0xfcc5e9, 0xadd7ff, 0xffffff, 0xa6accd,
        0xd0679d, 0x5de4c7, 0xfffac2, 0x89ddff, 0xfcc5e9, 0xadd7ff, 0xffffff,
    ]),
    fg: rgb(0xe4f0fb),
    bg: rgb(0x1b1e28),
    cursor: rgb(0xa6accd),
};

const VESPER: BuiltinScheme = BuiltinScheme {
    name: "Vesper",
    ansi: ansi16([
        0x101010, 0xf5a191, 0x90b99f, 0xe6b99d, 0xaca1cf, 0xe29eca, 0xea83a5, 0xb0b0b0, 0x7b7b7b,
        0xff8080, 0xa8d8b9, 0xffd1a6, 0xb9aeda, 0xf0b6d6, 0xf5a0c0, 0xffffff,
    ]),
    fg: rgb(0xb0b0b0),
    bg: rgb(0x101010),
    cursor: rgb(0xffc799),
};

const SONOKAI: BuiltinScheme = BuiltinScheme {
    name: "Sonokai",
    ansi: ansi16([
        0x181819, 0xfc5d7c, 0x9ed072, 0xe7c664, 0x76cce0, 0xb39df3, 0xf39660, 0xe2e2e3, 0x7f8490,
        0xfc5d7c, 0x9ed072, 0xe7c664, 0x76cce0, 0xb39df3, 0xf39660, 0xe2e2e3,
    ]),
    fg: rgb(0xe2e2e3),
    bg: rgb(0x2c2e34),
    cursor: rgb(0xe2e2e3),
};

const ONEDARK_PRO: BuiltinScheme = BuiltinScheme {
    name: "OneDark Pro",
    ansi: ansi16([
        0x282c34, 0xe06c75, 0x98c379, 0xd19a66, 0x61afef, 0xc678dd, 0x56b6c2, 0xabb2bf, 0x5c6370,
        0xe06c75, 0x98c379, 0xd19a66, 0x61afef, 0xc678dd, 0x56b6c2, 0xffffff,
    ]),
    fg: rgb(0xabb2bf),
    bg: rgb(0x282c34),
    cursor: rgb(0x528bff),
};

const MOONFLY: BuiltinScheme = BuiltinScheme {
    name: "Moonfly",
    ansi: ansi16([
        0x323437, 0xff5454, 0x8cc85f, 0xe3c78a, 0x80a0ff, 0xd183e8, 0x79dac8, 0xc6c6c6, 0x949494,
        0xff5189, 0xa0d17b, 0xf09479, 0x74b2ff, 0xd183e8, 0x85dc85, 0xe4e4e4,
    ]),
    fg: rgb(0xbdbdbd),
    bg: rgb(0x080808),
    cursor: rgb(0x9e9e9e),
};

const PAPERCOLOR_DARK: BuiltinScheme = BuiltinScheme {
    name: "PaperColor Dark",
    ansi: ansi16([
        0x1c1c1c, 0xaf005f, 0x5faf00, 0xd7af5f, 0x5fafd7, 0x808080, 0xd7875f, 0xd0d0d0, 0x585858,
        0x5faf5f, 0xafd700, 0xaf87d7, 0xffaf00, 0xff5faf, 0x00afaf, 0x5f8787,
    ]),
    fg: rgb(0xd0d0d0),
    bg: rgb(0x1c1c1c),
    cursor: rgb(0xd0d0d0),
};

const PAPERCOLOR_LIGHT: BuiltinScheme = BuiltinScheme {
    name: "PaperColor Light",
    ansi: ansi16([
        0xeeeeee, 0xaf0000, 0x008700, 0x5f8700, 0x0087af, 0x878787, 0x005f87, 0x444444, 0xbcbcbc,
        0xd70000, 0xd70087, 0x8700af, 0xd75f00, 0xd75f00, 0x005faf, 0x005f87,
    ]),
    fg: rgb(0x444444),
    bg: rgb(0xeeeeee),
    cursor: rgb(0x444444),
};

const OXOCARBON: BuiltinScheme = BuiltinScheme {
    name: "Oxocarbon",
    ansi: ansi16([
        0x262626, 0xee5396, 0x42be65, 0xffe97b, 0x33b1ff, 0xff7eb6, 0x3ddbd9, 0xdde1e6, 0x393939,
        0xee5396, 0x42be65, 0xffe97b, 0x33b1ff, 0xff7eb6, 0x3ddbd9, 0xf2f4f8,
    ]),
    fg: rgb(0xf2f4f8),
    bg: rgb(0x161616),
    cursor: rgb(0xf2f4f8),
};

const ANDROMEDA: BuiltinScheme = BuiltinScheme {
    name: "Andromeda",
    ansi: ansi16([
        0x000000, 0xee5d43, 0x96e072, 0xffe66d, 0x7cb7ff, 0xc74ded, 0x00e8c6, 0xd5ced9, 0x686868,
        0xee5d43, 0x96e072, 0xffe66d, 0x7cb7ff, 0xc74ded, 0x00e8c6, 0xffffff,
    ]),
    fg: rgb(0xd5ced9),
    bg: rgb(0x23262e),
    cursor: rgb(0xf8f8f0),
};

/// All built-in color schemes.
pub(crate) const BUILTIN_SCHEMES: &[&BuiltinScheme] = &[
    &CATPPUCCIN_MOCHA,
    &CATPPUCCIN_LATTE,
    &CATPPUCCIN_FRAPPE,
    &CATPPUCCIN_MACCHIATO,
    &ONE_DARK,
    &ONE_LIGHT,
    &SOLARIZED_DARK,
    &SOLARIZED_LIGHT,
    &DRACULA,
    &TOKYO_NIGHT,
    &TOKYO_NIGHT_STORM,
    &TOKYO_NIGHT_LIGHT,
    &WEZTERM_DEFAULT,
    &GRUVBOX_DARK,
    &GRUVBOX_LIGHT,
    &NORD,
    &ROSE_PINE,
    &ROSE_PINE_MOON,
    &ROSE_PINE_DAWN,
    &EVERFOREST_DARK,
    &EVERFOREST_LIGHT,
    &KANAGAWA,
    &KANAGAWA_LIGHT,
    &AYU_DARK,
    &AYU_MIRAGE,
    &AYU_LIGHT,
    &MATERIAL_DARK,
    &MATERIAL_LIGHT,
    &MONOKAI,
    &NIGHTFOX,
    &DAWNFOX,
    &CARBONFOX,
    &GITHUB_DARK,
    &GITHUB_LIGHT,
    &GITHUB_DARK_DIMMED,
    &SNAZZY,
    &TOMORROW_NIGHT,
    &TOMORROW_LIGHT,
    &ZENBURN,
    &ICEBERG_DARK,
    &ICEBERG_LIGHT,
    &NIGHT_OWL,
    &PALENIGHT,
    &HORIZON,
    &POIMANDRES,
    &VESPER,
    &SONOKAI,
    &ONEDARK_PRO,
    &MOONFLY,
    &PAPERCOLOR_DARK,
    &PAPERCOLOR_LIGHT,
    &OXOCARBON,
    &ANDROMEDA,
];
