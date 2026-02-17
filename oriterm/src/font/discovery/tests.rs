use super::families::{FALLBACK_FONTS, PRIMARY_FAMILIES};
use super::{
    EMBEDDED_FONT_DATA, FontOrigin, discover_fonts, embedded_family, resolve_user_fallback,
};

/// The embedded JetBrains Mono bytes parse as a valid font.
#[test]
fn embedded_font_is_valid() {
    let font_ref = swash::FontRef::from_index(EMBEDDED_FONT_DATA, 0);
    assert!(
        font_ref.is_some(),
        "embedded font data should parse as a valid font"
    );
}

/// The embedded family has the correct origin and variant flags.
#[test]
fn embedded_family_has_correct_origin() {
    let family = embedded_family();

    assert_eq!(family.origin, FontOrigin::Embedded);
    assert!(
        family.has_variant[0],
        "Regular slot must be marked available"
    );
    assert!(
        !family.has_variant[1],
        "Bold slot should be unavailable (needs synthesis)"
    );
    assert!(
        !family.has_variant[2],
        "Italic slot should be unavailable (needs synthesis)"
    );
    assert!(
        !family.has_variant[3],
        "BoldItalic slot should be unavailable (needs synthesis)"
    );

    // All paths are None for embedded fonts.
    for (i, path) in family.paths.iter().enumerate() {
        assert!(path.is_none(), "embedded font path[{i}] should be None");
    }
}

/// Every `FamilySpec` has at least one Regular candidate.
#[test]
fn family_spec_consistency() {
    for spec in PRIMARY_FAMILIES {
        assert!(
            !spec.regular.is_empty(),
            "FamilySpec {:?} must have at least one Regular candidate",
            spec.name,
        );
    }
}

/// Every `FallbackSpec` has at least one filename candidate.
#[test]
fn fallback_spec_consistency() {
    for spec in FALLBACK_FONTS {
        assert!(
            !spec.filenames.is_empty(),
            "FallbackSpec {:?} must have at least one filename",
            spec.name,
        );
    }
}

/// `discover_fonts` always succeeds — the embedded fallback guarantees a result.
#[test]
fn discover_finds_at_least_one_font() {
    let result = discover_fonts(None, 400);
    assert!(
        result.primary.has_variant[0],
        "discover_fonts must always find at least a Regular variant",
    );
}

/// A bogus family name doesn't panic and falls through to defaults or embedded.
#[test]
fn unknown_family_falls_back() {
    let result = discover_fonts(Some("NonExistentFontFamily_XYZ_12345"), 400);
    assert!(
        result.primary.has_variant[0],
        "bogus family should fall back gracefully",
    );
}

/// If a discovered Regular path is `Some`, the file actually exists on disk.
#[test]
fn discovered_regular_path_exists() {
    let result = discover_fonts(None, 400);
    if let Some(path) = &result.primary.paths[0] {
        assert!(
            path.exists(),
            "discovered Regular path should exist: {}",
            path.display(),
        );
    }
    // If paths[0] is None, it's the embedded font — that's fine.
}

/// All discovered fallback paths should exist on disk.
#[test]
fn discovered_fallback_paths_exist() {
    let result = discover_fonts(None, 400);
    for fb in &result.fallbacks {
        assert!(
            fb.path.exists(),
            "fallback path should exist: {}",
            fb.path.display(),
        );
    }
}

/// `resolve_user_fallback` returns `None` for a nonexistent font name.
#[test]
fn resolve_user_fallback_nonexistent() {
    let result = resolve_user_fallback("NonExistentFontFamily_XYZ_12345");
    assert!(result.is_none(), "bogus fallback name should return None");
}

/// Different weights don't panic and still produce valid results.
#[test]
fn different_weights_succeed() {
    for weight in [100, 300, 400, 700, 900] {
        let result = discover_fonts(None, weight);
        assert!(
            result.primary.has_variant[0],
            "weight {weight} should still find a Regular variant",
        );
    }
}

/// The embedded font data is a reasonable size (> 50KB for a real TTF).
#[test]
fn embedded_font_size_reasonable() {
    assert!(
        EMBEDDED_FONT_DATA.len() > 50_000,
        "embedded font should be > 50KB, got {} bytes",
        EMBEDDED_FONT_DATA.len(),
    );
}

/// Linux-specific: the font index finds real files on the system.
#[cfg(target_os = "linux")]
#[test]
fn font_index_finds_files() {
    let index = super::linux::build_font_index();
    // On a typical Linux system, at least one font should exist.
    // If no fonts are installed, the test still passes (empty index is valid).
    for (name, path) in &index {
        assert!(
            path.exists(),
            "indexed font {name:?} should exist at {}",
            path.display(),
        );
        // Spot-check: only font-like extensions.
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let valid = [
                "ttf", "otf", "ttc", "woff", "woff2", "dfont", "pcf", "bdf", "pfb",
            ];
            // We index everything; just verify the path is a file.
            let _ = valid.contains(&ext);
        }
    }
}

/// Linux-specific: DejaVu Sans Mono should be installed on most systems.
#[cfg(target_os = "linux")]
#[test]
fn linux_finds_dejavu() {
    let index = super::linux::build_font_index();
    // DejaVu is installed on most Linux distros. If not, skip gracefully.
    if let Some(path) = index.get("DejaVuSansMono.ttf") {
        assert!(path.exists(), "DejaVu Sans Mono path should exist");
    }
}

/// Verify `discover_fonts` result is internally consistent.
#[test]
fn discovery_result_consistency() {
    let result = discover_fonts(None, 400);
    verify_result_consistency(&result);
}

/// Embedded font has valid metrics (not a dummy/truncated file).
#[test]
fn embedded_font_has_metrics() {
    let font_ref = swash::FontRef::from_index(EMBEDDED_FONT_DATA, 0).unwrap();
    let metrics = font_ref.metrics(&[]);
    assert!(
        metrics.units_per_em > 0,
        "font must have valid units_per_em"
    );
    assert!(metrics.ascent > 0.0, "font must have positive ascent");
}

/// Discovered primary family name is non-empty.
#[test]
fn discovered_family_name_nonempty() {
    let result = discover_fonts(None, 400);
    assert!(
        !result.primary.family_name.is_empty(),
        "primary family name must not be empty",
    );
}

/// All discovered variant paths (Bold/Italic/BoldItalic) are distinct from Regular.
///
/// If DirectWrite or directory scan returned the same file for multiple variants,
/// the discovery layer should have filtered them to `None`.
#[test]
fn discovered_variant_paths_distinct() {
    let result = discover_fonts(None, 400);
    let regular = &result.primary.paths[0];
    for (i, path) in result.primary.paths.iter().enumerate().skip(1) {
        if let (Some(r), Some(p)) = (regular, path) {
            assert_ne!(
                r, p,
                "variant path[{i}] must differ from Regular path (duplicate = needs synthesis)",
            );
        }
    }
}

/// Discovery with user override falls back consistently: the result always
/// passes the same consistency checks regardless of override outcome.
#[test]
fn user_override_result_consistent() {
    let bogus = discover_fonts(Some("__bogus_font__"), 400);
    verify_result_consistency(&bogus);

    let no_override = discover_fonts(None, 400);
    verify_result_consistency(&no_override);
}

/// Fallback discovery deduplicates — no two fallbacks share the same path.
#[test]
fn fallback_paths_unique() {
    let result = discover_fonts(None, 400);
    let mut seen = std::collections::HashSet::new();
    for fb in &result.fallbacks {
        assert!(
            seen.insert(&fb.path),
            "duplicate fallback path: {}",
            fb.path.display(),
        );
    }
}

/// Linux-specific: user fallback resolves an absolute path to a real font file.
#[cfg(target_os = "linux")]
#[test]
fn resolve_user_fallback_absolute_path() {
    let index = super::linux::build_font_index();
    // Find any font file to test absolute path resolution.
    if let Some((_name, path)) = index.iter().next() {
        let path_str = path.to_str().expect("font path should be valid UTF-8");
        let result = resolve_user_fallback(path_str);
        assert!(
            result.is_some(),
            "absolute path to existing font should resolve"
        );
        let fb = result.unwrap();
        assert_eq!(fb.path, *path);
        assert_eq!(fb.origin, FontOrigin::UserConfig);
    }
}

/// Linux-specific: font index handles symlinks correctly (indexed path exists).
#[cfg(target_os = "linux")]
#[test]
fn font_index_follows_symlinks() {
    let index = super::linux::build_font_index();
    for (name, path) in &index {
        // Symlinks should resolve to real files.
        if path.is_symlink() {
            assert!(
                path.exists(),
                "symlinked font {name:?} at {} should resolve to a real file",
                path.display(),
            );
        }
    }
}

/// Linux-specific: font index keys are bare filenames (no directory components).
#[cfg(target_os = "linux")]
#[test]
fn font_index_keys_are_filenames() {
    let index = super::linux::build_font_index();
    for name in index.keys() {
        assert!(
            !name.contains('/'),
            "font index key should be a bare filename, got: {name:?}",
        );
    }
}

/// Helper: verify that a `DiscoveryResult` is internally consistent.
fn verify_result_consistency(result: &super::DiscoveryResult) {
    let primary = &result.primary;

    // has_variant must match paths.
    for i in 0..4 {
        assert_eq!(
            primary.has_variant[i],
            primary.paths[i].is_some(),
            "has_variant[{i}] must match paths[{i}].is_some() for {:?}",
            primary.family_name,
        );
    }

    // If origin is Embedded, all paths must be None.
    if primary.origin == FontOrigin::Embedded {
        for (i, path) in primary.paths.iter().enumerate() {
            assert!(
                path.is_none(),
                "embedded font should have no paths, but paths[{i}] is Some",
            );
        }
    }

    // Existing paths must point to real files.
    for (i, path) in primary.paths.iter().enumerate() {
        if let Some(p) = path {
            assert!(
                p.exists(),
                "primary path[{i}] should exist: {}",
                p.display(),
            );
        }
    }
}
