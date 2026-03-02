//! Write embedded shell integration scripts to disk with version stamping.
//!
//! Scripts are compiled into the binary via `include_str!` and written to the
//! user's data directory on first launch or after an upgrade. A `.version`
//! stamp prevents unnecessary rewrites when scripts are already up to date.

use std::io;
use std::path::{Path, PathBuf};

/// Write the embedded shell integration scripts to `base/shell-integration/`.
///
/// Returns the path to the `shell-integration/` directory on success.
/// Uses a version stamp to skip writes when scripts are already current.
pub(crate) fn ensure_scripts_on_disk(base: &Path) -> io::Result<PathBuf> {
    let dir = base.join("shell-integration");
    let version = env!("CARGO_PKG_VERSION");
    let stamp_path = dir.join(".version");

    // Skip if scripts are already written for this version.
    if let Ok(existing) = std::fs::read_to_string(&stamp_path) {
        if existing.trim() == version {
            return Ok(dir);
        }
    }

    write_bash_scripts(&dir)?;
    write_zsh_scripts(&dir)?;
    write_fish_scripts(&dir)?;
    write_powershell_scripts(&dir)?;

    // Stamp the version so subsequent launches skip writes.
    std::fs::write(&stamp_path, version)?;

    log::info!("shell_integration: scripts written to {}", dir.display());
    Ok(dir)
}

/// Write bash integration scripts.
fn write_bash_scripts(dir: &Path) -> io::Result<()> {
    let bash_dir = dir.join("bash");
    std::fs::create_dir_all(&bash_dir)?;
    std::fs::write(
        bash_dir.join("oriterm.bash"),
        include_str!("../../shell-integration/bash/oriterm.bash"),
    )?;
    std::fs::write(
        bash_dir.join("bash-preexec.sh"),
        include_str!("../../shell-integration/bash/bash-preexec.sh"),
    )
}

/// Write zsh integration scripts.
fn write_zsh_scripts(dir: &Path) -> io::Result<()> {
    let zsh_dir = dir.join("zsh");
    std::fs::create_dir_all(&zsh_dir)?;
    std::fs::write(
        zsh_dir.join(".zshenv"),
        include_str!("../../shell-integration/zsh/.zshenv"),
    )?;
    std::fs::write(
        zsh_dir.join("oriterm-integration"),
        include_str!("../../shell-integration/zsh/oriterm-integration"),
    )
}

/// Write fish integration scripts.
fn write_fish_scripts(dir: &Path) -> io::Result<()> {
    let fish_dir = dir.join("fish").join("vendor_conf.d");
    std::fs::create_dir_all(&fish_dir)?;
    std::fs::write(
        fish_dir.join("oriterm-shell-integration.fish"),
        include_str!("../../shell-integration/fish/vendor_conf.d/oriterm-shell-integration.fish"),
    )
}

/// Write `PowerShell` integration scripts.
fn write_powershell_scripts(dir: &Path) -> io::Result<()> {
    let ps_dir = dir.join("powershell");
    std::fs::create_dir_all(&ps_dir)?;
    std::fs::write(
        ps_dir.join("oriterm.ps1"),
        include_str!("../../shell-integration/powershell/oriterm.ps1"),
    )
}
