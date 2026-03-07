use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let assets = workspace_root.join("assets");

    // Version assembly — runs unconditionally on all platforms.
    let version = assemble_version();
    println!("cargo:rustc-env=ORITERM_VERSION={version}");

    // Rebuild when the git HEAD changes (new commit, branch switch).
    let git_head = workspace_root.join(".git/HEAD");
    if git_head.exists() {
        println!("cargo:rerun-if-changed={}", git_head.display());
        if let Ok(head_content) = std::fs::read_to_string(&git_head) {
            if let Some(ref_path) = head_content.trim().strip_prefix("ref: ") {
                let ref_file = workspace_root.join(".git").join(ref_path);
                if ref_file.exists() {
                    println!("cargo:rerun-if-changed={}", ref_file.display());
                }
            }
        }
    }
    println!("cargo:rerun-if-env-changed=ORITERM_CHANNEL");

    embed_icon(&out_dir, &assets);
    decode_icon_png(&out_dir, &assets);
}

/// Build the full version string.
///
/// Format: `{cargo_version}[-{channel}] ({hash} {date})`
///
/// Channel is derived from the `ORITERM_CHANNEL` env var:
/// - `"release"` -> no suffix (clean version).
/// - `"nightly"` -> `"-nightly"` suffix.
/// - unset/other -> `"-dev"` suffix.
///
/// If git is unavailable, the parenthetical shows `"(unknown)"`.
fn assemble_version() -> String {
    let base = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION must be set by Cargo");

    let channel = match std::env::var("ORITERM_CHANNEL").as_deref() {
        Ok("release") => "",
        Ok("nightly") => "-nightly",
        _ => "-dev",
    };

    let git_info = git_info().unwrap_or_else(|| "unknown".to_owned());

    format!("{base}{channel} ({git_info})")
}

/// Query git for short hash and commit date.
///
/// Returns `Some("abc1234 2026-03-07")` or `None` if git is unavailable.
fn git_info() -> Option<String> {
    let hash = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())?;

    let date = Command::new("git")
        .args(["show", "-s", "--format=%cs", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "unknown-date".to_owned(), |s| s.trim().to_owned());

    Some(format!("{hash} {date}"))
}

/// Embed the application icon into the Windows executable via `windres`.
fn embed_icon(out_dir: &str, assets: &Path) {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "windows" {
        return;
    }

    let rc_path = assets.join("icon.rc");
    let res_path = format!("{out_dir}/icon.res");

    let target = std::env::var("TARGET").unwrap_or_default();
    let windres = if target.contains("x86_64") && target.contains("gnu") {
        "x86_64-w64-mingw32-windres"
    } else {
        "windres"
    };

    let status = Command::new(windres)
        .args([
            "--include-dir",
            assets.to_str().unwrap(),
            rc_path.to_str().unwrap(),
            "-O",
            "coff",
            "-o",
            &res_path,
        ])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("cargo:rustc-link-arg-bins={res_path}");
            println!("cargo:rerun-if-changed={}", rc_path.display());
            println!(
                "cargo:rerun-if-changed={}",
                assets.join("icon.ico").display()
            );
            println!(
                "cargo:rerun-if-changed={}",
                assets.join("oriterm.manifest").display()
            );
        }
        Ok(s) => {
            eprintln!("warning: windres exited with {s}, exe will have no icon");
        }
        Err(e) => {
            eprintln!("warning: failed to run {windres}: {e}, exe will have no icon");
        }
    }
}

/// Decode PNG to raw RGBA at build time so runtime doesn't need the image crate.
fn decode_icon_png(out_dir: &str, assets: &Path) {
    let png_path = assets.join("icon-256.png");
    let png_bytes = std::fs::read(&png_path).expect("read assets/icon-256.png");
    let img = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png)
        .expect("decode icon PNG");
    let rgba = img.into_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut out = Vec::with_capacity(8 + rgba.len());
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&rgba);
    std::fs::write(format!("{out_dir}/icon_rgba.bin"), &out).expect("write icon_rgba.bin");
    println!("cargo:rerun-if-changed={}", png_path.display());
}
