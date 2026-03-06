//! WSL domain stub — spawns shells in a WSL distro.
//!
//! Currently always returns `can_spawn() = false` and `state() = Detached`.

use crate::DomainId;

use super::{Domain, DomainState};

/// Stub domain for WSL shell spawning.
///
/// Returns `can_spawn() = false` until the full WSL integration is
/// implemented (spawning `wsl.exe -d <distro> -- <shell>`).
#[allow(dead_code, reason = "used when WSL domain spawning is implemented")]
pub struct WslDomain {
    /// Domain identity.
    id: DomainId,
    /// WSL distribution name (e.g., `"Ubuntu"`).
    distro: String,
}

#[allow(dead_code, reason = "used when WSL domain spawning is implemented")]
impl WslDomain {
    /// Create a new WSL domain stub for the given distro.
    pub(crate) fn new(id: DomainId, distro: String) -> Self {
        Self { id, distro }
    }
}

impl Domain for WslDomain {
    fn id(&self) -> DomainId {
        self.id
    }

    fn name(&self) -> &str {
        &self.distro
    }

    fn state(&self) -> DomainState {
        DomainState::Detached
    }

    fn can_spawn(&self) -> bool {
        false
    }
}
