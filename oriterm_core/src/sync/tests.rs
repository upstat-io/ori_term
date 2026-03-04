use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use super::{FairMutex, FairMutexGuard};

#[test]
fn basic_lock_unlock() {
    let mutex = FairMutex::new(42);

    {
        let mut guard = mutex.lock();
        assert_eq!(*guard, 42);
        *guard = 100;
    }

    let guard = mutex.lock();
    assert_eq!(*guard, 100);
}

#[test]
fn two_threads_take_turns() {
    let mutex = Arc::new(FairMutex::new(Vec::new()));
    let iterations = 100;

    let m1 = Arc::clone(&mutex);
    let t1 = thread::spawn(move || {
        for i in 0..iterations {
            let mut guard = m1.lock();
            guard.push(('A', i));
        }
    });

    let m2 = Arc::clone(&mutex);
    let t2 = thread::spawn(move || {
        for i in 0..iterations {
            let mut guard = m2.lock();
            guard.push(('B', i));
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();

    let guard = mutex.lock();
    // Both threads contributed all their entries.
    assert_eq!(guard.len(), iterations * 2);

    let a_count = guard.iter().filter(|(c, _)| *c == 'A').count();
    let b_count = guard.iter().filter(|(c, _)| *c == 'B').count();
    assert_eq!(a_count, iterations);
    assert_eq!(b_count, iterations);
}

#[test]
fn try_lock_returns_none_when_locked() {
    let mutex = FairMutex::new(());
    let _guard = mutex.lock_unfair();

    assert!(mutex.try_lock().is_none());
}

#[test]
fn try_lock_succeeds_when_unlocked() {
    let mutex = FairMutex::new(7);
    let guard = mutex.try_lock();
    assert!(guard.is_some());
    assert_eq!(*guard.unwrap(), 7);
}

#[test]
fn lease_blocks_fair_lock() {
    let mutex = Arc::new(FairMutex::new(0));

    // Take a lease — this holds the `next` lock.
    let lease = mutex.lease();

    // Unfair lock should still succeed (bypasses `next`).
    {
        let mut guard = mutex.lock_unfair();
        *guard = 1;
    }

    // Fair lock from another thread should block because the lease holds `next`.
    let m = Arc::clone(&mutex);
    let handle = thread::spawn(move || {
        // This will block until the lease is dropped.
        let guard = m.lock();
        *guard
    });

    // Give the spawned thread time to attempt the lock.
    thread::sleep(Duration::from_millis(50));

    // The thread should still be running (blocked on `next`).
    assert!(!handle.is_finished());

    // Drop the lease — the spawned thread should now proceed.
    drop(lease);

    let val = handle.join().unwrap();
    assert_eq!(val, 1);
}

#[test]
fn lock_unfair_bypasses_fairness() {
    let mutex = FairMutex::new(42);

    // Lock unfair should give direct access to data.
    let guard = mutex.lock_unfair();
    assert_eq!(*guard, 42);
    drop(guard);

    // Fair lock should also work after unfair lock is released.
    let guard = mutex.lock();
    assert_eq!(*guard, 42);
}

#[test]
fn guard_deref_mut() {
    let mutex = FairMutex::new(String::from("hello"));
    let mut guard: FairMutexGuard<'_, String> = mutex.lock();
    guard.push_str(" world");
    assert_eq!(&*guard, "hello world");
}

// unlock_fair tests

#[test]
fn unlock_fair_releases_data() {
    let mutex = FairMutex::new(42);

    let mut guard = mutex.lock();
    *guard = 99;
    FairMutexGuard::unlock_fair(guard);

    // Lock should succeed immediately — both locks released.
    let guard = mutex.lock();
    assert_eq!(*guard, 99);
}

#[test]
fn unlock_fair_hands_off_to_waiter() {
    let mutex = Arc::new(FairMutex::new(Vec::<char>::new()));

    // Thread A holds the lock.
    let mut guard = mutex.lock();
    guard.push('A');

    // Thread B starts and blocks on lock().
    let m = Arc::clone(&mutex);
    let handle = thread::spawn(move || {
        let mut g = m.lock();
        g.push('B');
    });

    // Let thread B park on the fairness gate.
    thread::sleep(Duration::from_millis(20));
    assert!(!handle.is_finished());

    // Fair-unlock guarantees B gets the handoff.
    FairMutexGuard::unlock_fair(guard);
    handle.join().unwrap();

    let g = mutex.lock();
    assert_eq!(*g, vec!['A', 'B']);
}

/// Simulates PTY reader vs render thread contention.
///
/// The "reader" runs a tight `lock`/`unlock_fair` loop. The "renderer" runs
/// a tight `lock`/`drop` loop. Over a fixed time window, both threads must
/// get substantial access. Without `unlock_fair`, the reader would starve
/// the renderer through `parking_lot`'s barging behavior.
#[test]
fn unlock_fair_prevents_starvation() {
    let mutex = Arc::new(FairMutex::new(()));
    let reader_count = Arc::new(AtomicUsize::new(0));
    let renderer_count = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    // "Reader" thread — tight loop with fair unlock (simulates PTY reader).
    let mx = Arc::clone(&mutex);
    let rc = Arc::clone(&reader_count);
    let dn = Arc::clone(&done);
    let br = Arc::clone(&barrier);
    let reader = thread::spawn(move || {
        br.wait();
        while !dn.load(Ordering::Relaxed) {
            let guard = mx.lock();
            rc.fetch_add(1, Ordering::Relaxed);
            FairMutexGuard::unlock_fair(guard);
        }
    });

    // "Renderer" thread — tight loop with regular drop (simulates render).
    let mx = Arc::clone(&mutex);
    let wc = Arc::clone(&renderer_count);
    let dn = Arc::clone(&done);
    let br = Arc::clone(&barrier);
    let renderer = thread::spawn(move || {
        br.wait();
        while !dn.load(Ordering::Relaxed) {
            let guard = mx.lock();
            wc.fetch_add(1, Ordering::Relaxed);
            drop(guard);
        }
    });

    thread::sleep(Duration::from_millis(200));
    done.store(true, Ordering::Relaxed);

    reader.join().unwrap();
    renderer.join().unwrap();

    let rd = reader_count.load(Ordering::Relaxed);
    let rn = renderer_count.load(Ordering::Relaxed);

    // With fair unlock, the renderer should get at least 20% of total
    // acquisitions. Without it, the renderer typically gets < 1%.
    let total = rd + rn;
    let renderer_pct = if total > 0 { rn * 100 / total } else { 0 };
    assert!(
        renderer_pct >= 20,
        "renderer starved: reader={rd}, renderer={rn} ({renderer_pct}% renderer)",
    );
}

/// Measures that `unlock_fair` has negligible overhead vs regular `drop`
/// when uncontested (single-thread, no waiters).
#[test]
fn unlock_fair_uncontested_throughput() {
    let iterations = 200_000;

    // Regular drop baseline.
    let mutex_a = FairMutex::new(0u64);
    let start = Instant::now();
    for _ in 0..iterations {
        let mut g = mutex_a.lock();
        *g += 1;
    }
    let drop_elapsed = start.elapsed();

    // Fair unlock.
    let mutex_b = FairMutex::new(0u64);
    let start = Instant::now();
    for _ in 0..iterations {
        let mut g = mutex_b.lock();
        *g += 1;
        FairMutexGuard::unlock_fair(g);
    }
    let fair_elapsed = start.elapsed();

    assert_eq!(*mutex_a.lock(), iterations);
    assert_eq!(*mutex_b.lock(), iterations);

    // Fair unlock should be within 3x of regular drop when uncontested.
    // In practice they're nearly identical — unlock_fair with no waiters
    // is just a regular unlock.
    assert!(
        fair_elapsed < drop_elapsed * 3,
        "fair unlock too slow: fair={fair_elapsed:?}, drop={drop_elapsed:?}",
    );
}

// Locking strategy comparison

/// Busy-waits for approximately the given duration.
///
/// Uses a tight loop checking `Instant::elapsed()` rather than
/// `thread::sleep` to simulate CPU-bound work (like VTE parsing) without
/// yielding to the OS scheduler.
fn busy_wait(duration: Duration) {
    let start = Instant::now();
    while start.elapsed() < duration {
        std::hint::spin_loop();
    }
}

/// Results from a contention benchmark run.
struct ContentionResult {
    reader_count: usize,
    renderer_count: usize,
}

impl ContentionResult {
    /// Renderer share as a percentage of total acquisitions.
    fn renderer_pct(&self) -> usize {
        let total = self.reader_count + self.renderer_count;
        if total > 0 {
            self.renderer_count * 100 / total
        } else {
            0
        }
    }

    /// Renderer lock acquisitions per second.
    fn renderer_rate(&self, duration: Duration) -> f64 {
        self.renderer_count as f64 / duration.as_secs_f64()
    }
}

/// Runs a contention benchmark: a reader thread and renderer thread compete
/// for the same `FairMutex` over `duration`. The `reader_body` closure
/// defines how the reader acquires, holds, and releases the lock.
fn run_contention_bench<F>(duration: Duration, reader_body: F) -> ContentionResult
where
    F: Fn(&FairMutex<()>) + Send + 'static,
{
    let mutex = Arc::new(FairMutex::new(()));
    let reader_count = Arc::new(AtomicUsize::new(0));
    let renderer_count = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let mx = Arc::clone(&mutex);
    let rc = Arc::clone(&reader_count);
    let dn = Arc::clone(&done);
    let br = Arc::clone(&barrier);
    let reader = thread::spawn(move || {
        br.wait();
        while !dn.load(Ordering::Relaxed) {
            reader_body(&mx);
            rc.fetch_add(1, Ordering::Relaxed);
        }
    });

    let mx = Arc::clone(&mutex);
    let wc = Arc::clone(&renderer_count);
    let dn = Arc::clone(&done);
    let br = Arc::clone(&barrier);
    let renderer = thread::spawn(move || {
        br.wait();
        while !dn.load(Ordering::Relaxed) {
            let guard = mx.lock();
            wc.fetch_add(1, Ordering::Relaxed);
            drop(guard);
        }
    });

    thread::sleep(duration);
    done.store(true, Ordering::Relaxed);
    reader.join().unwrap();
    renderer.join().unwrap();

    ContentionResult {
        reader_count: reader_count.load(Ordering::Relaxed),
        renderer_count: renderer_count.load(Ordering::Relaxed),
    }
}

/// Compares contention behavior of two locking strategies.
///
/// **Pattern A** (pre-04f58ab baseline): `lease()` + `lock_unfair()` + drop.
/// The PTY reader holds the fairness gate via a lease, then acquires data
/// with `lock_unfair`. The renderer uses `lock()` (fair). The lease prevents
/// the renderer from acquiring the fairness gate during reader work.
///
/// **Pattern B** (current, post-04f58ab): `lock()` + `unlock_fair()`.
/// The PTY reader uses fair locking and explicitly hands off via
/// `unlock_fair`, giving the renderer a guaranteed turn between each chunk.
///
/// Both patterns simulate ~50us of CPU-bound work per lock acquisition
/// (approximating VTE parsing of a 64KB chunk). The renderer does minimal
/// work (just a counter increment) to simulate frame-rate polling.
#[test]
fn compare_locking_strategies() {
    let duration = Duration::from_millis(500);
    let work = Duration::from_micros(50);

    // Pattern A: lease + lock_unfair (baseline).
    let work_a = work;
    let result_a = run_contention_bench(duration, move |mutex| {
        let _lease = mutex.lease();
        let guard = mutex.lock_unfair();
        busy_wait(work_a);
        drop(guard);
    });

    // Pattern B: lock + unlock_fair (current).
    let work_b = work;
    let result_b = run_contention_bench(duration, move |mutex| {
        let guard = mutex.lock();
        busy_wait(work_b);
        FairMutexGuard::unlock_fair(guard);
    });

    let rate_a = result_a.renderer_rate(duration);
    let rate_b = result_b.renderer_rate(duration);

    eprintln!();
    eprintln!(
        "=== Locking Strategy Comparison ({}ms, {}us work) ===",
        duration.as_millis(),
        work.as_micros()
    );
    eprintln!();
    eprintln!("Pattern A (lease + lock_unfair, baseline):");
    eprintln!("  reader:   {:>8} acquisitions", result_a.reader_count);
    eprintln!(
        "  renderer: {:>8} acquisitions ({}% share)",
        result_a.renderer_count,
        result_a.renderer_pct()
    );
    eprintln!("  renderer rate: {rate_a:.0} locks/sec");
    eprintln!();
    eprintln!("Pattern B (lock + unlock_fair, current):");
    eprintln!("  reader:   {:>8} acquisitions", result_b.reader_count);
    eprintln!(
        "  renderer: {:>8} acquisitions ({}% share)",
        result_b.renderer_count,
        result_b.renderer_pct()
    );
    eprintln!("  renderer rate: {rate_b:.0} locks/sec");
    eprintln!();
    if rate_a > 0.0 {
        eprintln!("Renderer access improvement: {:.1}x", rate_b / rate_a);
    }
    eprintln!();

    // Pattern B should give the renderer substantial access. The renderer
    // gets turns between each reader chunk because `unlock_fair` hands off
    // the fairness gate.
    assert!(
        result_b.renderer_pct() >= 20,
        "Pattern B renderer share too low: {}% (expected >= 20%)",
        result_b.renderer_pct(),
    );
}

// take_contended tests

#[test]
fn take_contended_initially_false() {
    let mutex = FairMutex::new(());
    assert!(!mutex.take_contended());
}

#[test]
fn take_contended_cleared_after_read() {
    let mutex = Arc::new(FairMutex::new(()));

    // Hold the lock so a second thread blocks on the fairness gate.
    let guard = mutex.lock();

    let m = Arc::clone(&mutex);
    let handle = thread::spawn(move || {
        let _g = m.lock();
    });

    // Let the spawned thread park on the fairness gate.
    thread::sleep(Duration::from_millis(20));

    // First take should return true (thread blocked).
    assert!(mutex.take_contended());
    // Second take should return false (flag cleared).
    assert!(!mutex.take_contended());

    drop(guard);
    handle.join().unwrap();
}

#[test]
fn take_contended_not_set_on_unblocked_lock() {
    let mutex = FairMutex::new(());

    // Nobody holds the lock, so lock() should not set contended.
    {
        let _g = mutex.lock();
    }
    assert!(
        !mutex.take_contended(),
        "contended should be false when lock() doesn't block"
    );

    // Multiple uncontested acquisitions should not set it.
    for _ in 0..10 {
        let _g = mutex.lock();
    }
    assert!(!mutex.take_contended());
}

#[test]
fn take_contended_set_on_blocked_lock() {
    let mutex = Arc::new(FairMutex::new(()));

    // Thread A holds the fairness gate.
    let guard = mutex.lock();

    // Thread B blocks on the fairness gate → sets contended.
    let m = Arc::clone(&mutex);
    let handle = thread::spawn(move || {
        let _g = m.lock();
    });

    // Let thread B attempt the lock and park.
    thread::sleep(Duration::from_millis(20));

    assert!(
        mutex.take_contended(),
        "contended should be true when a lock() caller blocked"
    );

    drop(guard);
    handle.join().unwrap();
}

#[test]
fn take_contended_resets_per_contention_event() {
    let mutex = Arc::new(FairMutex::new(()));

    // Round 1: cause contention, read, clear.
    let guard = mutex.lock();
    let m = Arc::clone(&mutex);
    let h1 = thread::spawn(move || {
        let _g = m.lock();
    });
    thread::sleep(Duration::from_millis(20));
    assert!(mutex.take_contended());
    drop(guard);
    h1.join().unwrap();

    // Between rounds: no contention.
    assert!(!mutex.take_contended());

    // Round 2: cause contention again, verify flag is set again.
    let guard = mutex.lock();
    let m = Arc::clone(&mutex);
    let h2 = thread::spawn(move || {
        let _g = m.lock();
    });
    thread::sleep(Duration::from_millis(50));
    assert!(
        mutex.take_contended(),
        "contended should be re-set on new contention"
    );
    drop(guard);
    h2.join().unwrap();
}
