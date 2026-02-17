//! Synchronization primitives for terminal emulation.
//!
//! Provides [`FairMutex`], a mutex that prevents starvation between the PTY
//! reader thread and the render thread. Two threads competing for the same
//! lock will alternate access rather than allowing one to monopolize it.

use std::ops::{Deref, DerefMut};

use parking_lot::{Mutex, MutexGuard};

/// A fair mutex that prevents thread starvation.
///
/// Uses a two-lock protocol: a `next` lock for queuing and a `data` lock for
/// the protected value. Fair callers acquire `next` first (establishing FIFO
/// order), then `data`. Unfair callers bypass `next` entirely.
///
/// The PTY reader thread typically uses [`lock_unfair`](FairMutex::lock_unfair)
/// or [`try_lock_unfair`](FairMutex::try_lock_unfair) for throughput, while
/// the render thread uses [`lock`](FairMutex::lock) for guaranteed access.
pub struct FairMutex<T> {
    /// The protected data.
    data: Mutex<T>,
    /// Fairness gate — establishes FIFO ordering among fair callers.
    next: Mutex<()>,
}

/// RAII guard returned by [`FairMutex::lock`].
///
/// Holds both the fairness gate and data lock. Releasing this guard frees
/// both, allowing the next queued fair caller to proceed.
pub struct FairMutexGuard<'a, T> {
    /// Data lock — provides access to the protected value.
    data: MutexGuard<'a, T>,
    /// Fairness gate — held to prevent queue jumping.
    _next: MutexGuard<'a, ()>,
}

/// RAII lease on the fairness gate, returned by [`FairMutex::lease`].
///
/// Reserves a position in the fair queue without locking the data. Useful
/// when the PTY reader thread needs to signal intent to access the terminal
/// state, preventing the render thread from starving it.
pub struct FairMutexLease<'a> {
    _next: MutexGuard<'a, ()>,
}

impl<T> FairMutex<T> {
    /// Creates a new `FairMutex` protecting `data`.
    pub fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
        }
    }

    /// Acquires the mutex fairly.
    ///
    /// Blocks until both the fairness gate and data lock are available,
    /// guaranteeing FIFO ordering among fair callers. The returned guard
    /// holds both locks until dropped.
    pub fn lock(&self) -> FairMutexGuard<'_, T> {
        let next = self.next.lock();
        let data = self.data.lock();
        FairMutexGuard { data, _next: next }
    }

    /// Acquires the mutex without fairness.
    ///
    /// Bypasses the fairness gate, directly contending for the data lock.
    /// Use this in the PTY reader thread where throughput matters more than
    /// fairness.
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Attempts to acquire the mutex without fairness or blocking.
    ///
    /// Returns `None` if the data lock is currently held. Does not contend
    /// for the fairness gate.
    pub fn try_lock_unfair(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }

    /// Reserves a position in the fair queue without locking the data.
    ///
    /// The returned [`FairMutexLease`] holds the fairness gate, preventing
    /// other fair callers from proceeding until the lease is dropped. This
    /// is useful when the PTY reader thread needs to perform multiple
    /// operations and wants to ensure it isn't starved between them.
    pub fn lease(&self) -> FairMutexLease<'_> {
        FairMutexLease {
            _next: self.next.lock(),
        }
    }
}

impl<T> Deref for FairMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> DerefMut for FairMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

#[cfg(test)]
mod tests;
