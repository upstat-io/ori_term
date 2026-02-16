//! Per-platform font family and fallback constants.
//!
//! Pure data — no logic, no I/O. Each platform defines its preferred primary
//! families (tried in order) and fallback fonts for missing-glyph coverage.

/// A font family with candidate filenames for each style variant.
///
/// Filenames are tried in order within each variant slot. On Windows, these
/// are full paths; on Linux/macOS, bare filenames resolved via directory scan.
pub(crate) struct FamilySpec {
    /// Human-readable family name (for logging).
    pub name: &'static str,
    /// Candidate filenames for the Regular variant.
    pub regular: &'static [&'static str],
    /// Candidate filenames for the Bold variant.
    pub bold: &'static [&'static str],
    /// Candidate filenames for the Italic variant.
    pub italic: &'static [&'static str],
    /// Candidate filenames for the Bold Italic variant.
    pub bold_italic: &'static [&'static str],
}

/// A single fallback font for missing-glyph coverage.
pub(crate) struct FallbackSpec {
    /// Human-readable name (for logging).
    pub name: &'static str,
    /// Candidate filenames to search for.
    pub filenames: &'static [&'static str],
}

// Windows: full paths to C:\Windows\Fonts\.

#[cfg(target_os = "windows")]
pub(crate) const PRIMARY_FAMILIES: &[FamilySpec] = &[
    FamilySpec {
        name: "JetBrains Mono",
        regular: &[r"C:\Windows\Fonts\JetBrainsMono-Regular.ttf"],
        bold: &[r"C:\Windows\Fonts\JetBrainsMono-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\JetBrainsMono-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\JetBrainsMono-BoldItalic.ttf"],
    },
    FamilySpec {
        name: "JetBrainsMono Nerd Font",
        regular: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Regular.ttf"],
        bold: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\JetBrainsMonoNerdFont-BoldItalic.ttf"],
    },
    FamilySpec {
        name: "Cascadia Mono NF",
        regular: &[r"C:\Windows\Fonts\CascadiaMonoNF.ttf"],
        bold: &[r"C:\Windows\Fonts\CascadiaMonoNF-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\CascadiaMonoNF-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\CascadiaMonoNF-BoldItalic.ttf"],
    },
    FamilySpec {
        name: "Cascadia Mono",
        regular: &[r"C:\Windows\Fonts\CascadiaMono.ttf"],
        bold: &[r"C:\Windows\Fonts\CascadiaMono-Bold.ttf"],
        italic: &[r"C:\Windows\Fonts\CascadiaMono-Italic.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\CascadiaMono-BoldItalic.ttf"],
    },
    FamilySpec {
        name: "Consolas",
        regular: &[r"C:\Windows\Fonts\consola.ttf"],
        bold: &[r"C:\Windows\Fonts\consolab.ttf"],
        italic: &[r"C:\Windows\Fonts\consolai.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\consolaz.ttf"],
    },
    FamilySpec {
        name: "Courier New",
        regular: &[r"C:\Windows\Fonts\cour.ttf"],
        bold: &[r"C:\Windows\Fonts\courbd.ttf"],
        italic: &[r"C:\Windows\Fonts\couri.ttf"],
        bold_italic: &[r"C:\Windows\Fonts\courbi.ttf"],
    },
];

/// DirectWrite family names to try, in priority order.
#[cfg(target_os = "windows")]
pub(crate) const DWRITE_FAMILY_NAMES: &[&str] = &[
    "JetBrains Mono",
    "JetBrainsMono Nerd Font",
    "Cascadia Mono NF",
    "Cascadia Mono",
    "Consolas",
    "Courier New",
];

/// DirectWrite fallback family names for missing-glyph coverage.
#[cfg(target_os = "windows")]
pub(crate) const DWRITE_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI Symbol", "MS Gothic", "Segoe UI"];

#[cfg(target_os = "windows")]
pub(crate) const FALLBACK_FONTS: &[FallbackSpec] = &[
    FallbackSpec {
        name: "Segoe UI Symbol",
        filenames: &[r"C:\Windows\Fonts\seguisym.ttf"],
    },
    FallbackSpec {
        name: "MS Gothic",
        filenames: &[r"C:\Windows\Fonts\msgothic.ttc"],
    },
    FallbackSpec {
        name: "Segoe UI",
        filenames: &[r"C:\Windows\Fonts\segoeui.ttf"],
    },
];

// Linux: bare filenames resolved via directory scan.

#[cfg(target_os = "linux")]
pub(crate) const PRIMARY_FAMILIES: &[FamilySpec] = &[
    FamilySpec {
        name: "JetBrains Mono",
        regular: &[
            "JetBrainsMono-Regular.ttf",
            "JetBrainsMonoNerdFont-Regular.ttf",
        ],
        bold: &["JetBrainsMono-Bold.ttf", "JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &[
            "JetBrainsMono-Italic.ttf",
            "JetBrainsMonoNerdFont-Italic.ttf",
        ],
        bold_italic: &[
            "JetBrainsMono-BoldItalic.ttf",
            "JetBrainsMonoNerdFont-BoldItalic.ttf",
        ],
    },
    FamilySpec {
        name: "Ubuntu Mono",
        regular: &["UbuntuMono-Regular.ttf", "UbuntuMonoNerdFont-Regular.ttf"],
        bold: &["UbuntuMono-Bold.ttf", "UbuntuMonoNerdFont-Bold.ttf"],
        italic: &["UbuntuMono-Italic.ttf", "UbuntuMonoNerdFont-Italic.ttf"],
        bold_italic: &[
            "UbuntuMono-BoldItalic.ttf",
            "UbuntuMonoNerdFont-BoldItalic.ttf",
        ],
    },
    FamilySpec {
        name: "DejaVu Sans Mono",
        regular: &["DejaVuSansMono.ttf"],
        bold: &["DejaVuSansMono-Bold.ttf"],
        italic: &["DejaVuSansMono-Oblique.ttf"],
        bold_italic: &["DejaVuSansMono-BoldOblique.ttf"],
    },
    FamilySpec {
        name: "Liberation Mono",
        regular: &["LiberationMono-Regular.ttf"],
        bold: &["LiberationMono-Bold.ttf"],
        italic: &["LiberationMono-Italic.ttf"],
        bold_italic: &["LiberationMono-BoldItalic.ttf"],
    },
];

#[cfg(target_os = "linux")]
pub(crate) const FALLBACK_FONTS: &[FallbackSpec] = &[
    FallbackSpec {
        name: "Noto Sans Mono",
        filenames: &["NotoSansMono-Regular.ttf"],
    },
    FallbackSpec {
        name: "Noto Sans Symbols2",
        filenames: &["NotoSansSymbols2-Regular.ttf"],
    },
    FallbackSpec {
        name: "Noto Sans CJK",
        filenames: &["NotoSansCJK-Regular.ttc", "NotoSansCJKsc-Regular.otf"],
    },
    FallbackSpec {
        name: "DejaVu Sans",
        filenames: &["DejaVuSans.ttf"],
    },
];

// macOS: bare filenames resolved via directory scan.

#[cfg(target_os = "macos")]
pub(crate) const PRIMARY_FAMILIES: &[FamilySpec] = &[
    FamilySpec {
        name: "JetBrains Mono",
        regular: &[
            "JetBrainsMono-Regular.ttf",
            "JetBrainsMonoNerdFont-Regular.ttf",
        ],
        bold: &["JetBrainsMono-Bold.ttf", "JetBrainsMonoNerdFont-Bold.ttf"],
        italic: &[
            "JetBrainsMono-Italic.ttf",
            "JetBrainsMonoNerdFont-Italic.ttf",
        ],
        bold_italic: &[
            "JetBrainsMono-BoldItalic.ttf",
            "JetBrainsMonoNerdFont-BoldItalic.ttf",
        ],
    },
    FamilySpec {
        name: "SF Mono",
        regular: &["SFMono-Regular.otf"],
        bold: &["SFMono-Bold.otf"],
        italic: &["SFMono-RegularItalic.otf"],
        bold_italic: &["SFMono-BoldItalic.otf"],
    },
    FamilySpec {
        name: "Menlo",
        regular: &["Menlo-Regular.ttc", "Menlo.ttc"],
        bold: &["Menlo-Bold.ttc"],
        italic: &["Menlo-Italic.ttc"],
        bold_italic: &["Menlo-BoldItalic.ttc"],
    },
    FamilySpec {
        name: "Monaco",
        regular: &["Monaco.ttf", "Monaco.dfont"],
        bold: &[],
        italic: &[],
        bold_italic: &[],
    },
    FamilySpec {
        name: "Courier New",
        regular: &["Courier New.ttf"],
        bold: &["Courier New Bold.ttf"],
        italic: &["Courier New Italic.ttf"],
        bold_italic: &["Courier New Bold Italic.ttf"],
    },
];

#[cfg(target_os = "macos")]
pub(crate) const FALLBACK_FONTS: &[FallbackSpec] = &[
    FallbackSpec {
        name: "Apple Symbols",
        filenames: &["Apple Symbols.ttf"],
    },
    FallbackSpec {
        name: "Hiragino Sans",
        filenames: &["HiraginoSans-W3.ttc", "ヒラギノ角ゴシック W3.ttc"],
    },
    FallbackSpec {
        name: "Apple Color Emoji",
        filenames: &["Apple Color Emoji.ttc"],
    },
];
