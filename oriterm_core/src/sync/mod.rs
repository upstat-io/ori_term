//! Synchronization primitives for terminal emulation.
//!
//! Provides [`FairMutex`], a mutex that prevents starvation between the PTY
//! reader thread and the render thread.
//!
//! **Usage pattern** (matches Alacritty):
//! - **PTY reader**: Holds a [`lease`](FairMutex::lease) during its read+parse
//!   cycle, preventing the renderer from starting a fair lock. Uses
//!   [`try_lock`](FairMutex::try_lock) or
//!   [`lock_unfair`](FairMutex::lock_unfair) for the data lock (both bypass
//!   the fairness gate, which the reader already holds via the lease).
//! - **Renderer**: Uses [`lock`](FairMutex::lock) (fair) to acquire the
//!   terminal. Blocks until the reader's lease is released, then gets
//!   guaranteed access.

use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, MutexGuard};

/// A fair mutex that prevents thread starvation.
///
/// Uses a two-lock protocol: a `next` lock for queuing and a `data` lock for
/// the protected value. Fair callers acquire `next` first (establishing FIFO
/// order), then `data`. Unfair callers bypass `next` entirely.
///
/// The PTY reader holds a [`lease`](Self::lease) and uses
/// [`try_lock`](Self::try_lock) or [`lock_unfair`](Self::lock_unfair). The
/// renderer uses [`lock`](Self::lock) (fair). This ensures the renderer gets
/// a turn between reader cycles while the reader always drains the PTY.
pub struct FairMutex<T> {
    /// The protected data.
    data: Mutex<T>,
    /// Fairness gate — establishes FIFO ordering among fair callers.
    next: Mutex<()>,
    /// Set when a `lock()` caller had to wait for the fairness gate.
    /// Cleared by `take_contended()`. The PTY reader checks this to
    /// decide whether to coalesce (sleep) — only when the renderer
    /// actually blocked does the reader yield.
    contended: AtomicBool,
}

/// RAII guard returned by [`FairMutex::lock`].
///
/// Holds both the fairness gate and data lock. Releasing this guard frees
/// both, allowing the next queued fair caller to proceed.
pub struct FairMutexGuard<'a, T> {
    /// Data lock — provides access to the protected value.
    data: MutexGuard<'a, T>,
    /// Fairness gate — held to prevent queue jumping.
    next: MutexGuard<'a, ()>,
}

/// RAII lease on the fairness gate, returned by [`FairMutex::lease`].
///
/// Reserves a position in the fair queue without locking the data. While
/// held, fair callers ([`lock`](FairMutex::lock)) cannot acquire the
/// fairness gate. The PTY reader holds this during its entire read+parse
/// cycle so the renderer waits until the reader explicitly yields.
pub struct FairMutexLease<'a> {
    /// Held for Drop — releasing this guard frees the fairness gate.
    _next: MutexGuard<'a, ()>,
}

impl<T> FairMutex<T> {
    /// Creates a new `FairMutex` protecting `data`.
    pub fn new(data: T) -> Self {
        Self {
            data: Mutex::new(data),
            next: Mutex::new(()),
            contended: AtomicBool::new(false),
        }
    }

    /// Acquires the mutex fairly.
    ///
    /// Blocks until both the fairness gate and data lock are available,
    /// guaranteeing FIFO ordering among fair callers. If the fairness
    /// gate is already held (by a reader's [`lease`](Self::lease)), sets
    /// the `contended` flag before blocking so the holder can detect
    /// contention via [`take_contended`](Self::take_contended).
    pub fn lock(&self) -> FairMutexGuard<'_, T> {
        let next = if let Some(guard) = self.next.try_lock() {
            guard
        } else {
            self.contended.store(true, Ordering::Release);
            self.next.lock()
        };
        let data = self.data.lock();
        FairMutexGuard { data, next }
    }

    /// Returns `true` if any `lock()` call blocked since the last check,
    /// and clears the flag.
    ///
    /// The PTY reader calls this after each processing cycle to decide
    /// whether to yield. When the renderer had to wait for the fairness
    /// gate, this returns `true` once, signaling the reader to coalesce.
    pub fn take_contended(&self) -> bool {
        self.contended.swap(false, Ordering::Acquire)
    }

    /// Acquires the data lock without going through the fairness gate.
    ///
    /// Blocks until the data lock is available but does NOT acquire the
    /// fairness gate. Used by the PTY reader when its buffer is full and
    /// it must parse — the reader already holds the fairness gate via a
    /// [`lease`](Self::lease).
    ///
    /// **Do not use from the renderer** — use [`lock`](Self::lock) instead
    /// to ensure fair access.
    pub fn lock_unfair(&self) -> MutexGuard<'_, T> {
        self.data.lock()
    }

    /// Attempts to acquire the data lock without blocking.
    ///
    /// Returns `None` if the data lock is currently held. Bypasses the
    /// fairness gate — used by the PTY reader to check if the renderer
    /// has released the terminal. If `None`, the reader continues
    /// buffering PTY data and retries on the next iteration.
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.data.try_lock()
    }

    /// Reserves the fairness gate without locking the data.
    ///
    /// The PTY reader holds the returned [`FairMutexLease`] during its
    /// entire read+parse cycle. While the lease is held, the renderer's
    /// [`lock`](Self::lock) blocks on the fairness gate — ensuring the
    /// renderer waits for the reader to finish its cycle before acquiring
    /// the terminal.
    ///
    /// The reader uses [`try_lock`](Self::try_lock) or
    /// [`lock_unfair`](Self::lock_unfair) for the data lock while
    /// holding the lease (both bypass the fairness gate).
    pub fn lease(&self) -> FairMutexLease<'_> {
        FairMutexLease {
            _next: self.next.lock(),
        }
    }
}

impl<T> FairMutexGuard<'_, T> {
    /// Releases the guard using `parking_lot`'s fair unlock protocol.
    ///
    /// Unlike regular `drop()`, this hands the fairness gate directly to the
    /// next waiting thread (if any), preventing barging.
    ///
    /// When no thread is waiting, this behaves identically to `drop()`.
    pub fn unlock_fair(self) {
        let Self { data, next } = self;
        // Release data first so the next thread can acquire it immediately
        // after receiving the fairness gate handoff.
        drop(data);
        MutexGuard::unlock_fair(next);
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
