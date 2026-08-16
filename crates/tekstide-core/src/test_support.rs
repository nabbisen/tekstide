use std::sync::{Condvar, Mutex, OnceLock};

/// Caps how many real-process-spawning tests, across the whole crate,
/// run their spawn-through-cleanup critical section at once, regardless
/// of `--test-threads`.
///
/// Real subprocess spawning inside a heavily multi-threaded test binary
/// gets measurably slower as the binary's total thread count grows --
/// `fork()`'s well-known cost scaling with the parent process's thread
/// count. The first version of this measurement (see review request
/// 212) was confounded by a real, separate bug in one of
/// `runtime::terminal::reader::tests`' own tests (channel starvation
/// while waiting for a final wake without draining in between -- see
/// `transcript_written_through_the_reader_thread_is_byte_identical_to_pty_output`'s
/// own doc comment), which failed at low thread counts for a reason
/// unrelated to contention. **Corrected measurement, that bug fixed**,
/// `cargo test -p tekstide-core --lib runtime::terminal::reader::tests::`,
/// isolated, no other workspace tests running:
///
/// | `--test-threads`            | failures |
/// |---|---|
/// | 2                            | 0/8 |
/// | 4                            | 0/8 |
/// | 8                            | 0/8, then 0/20 on a larger sample |
/// | 16 (this machine's default)  | 6/8 |
///
/// **6 is the cap**: below the clean 8-thread measurement for margin,
/// and confirmed clean itself at 0/30 default-concurrency runs of that
/// file plus 0/25 runs of the full `cargo test --workspace
/// --all-targets --all-features` gate. Response 212 independently
/// reproduced the original (confounded) shape and found the *identity*
/// of the failing test changing between repeats -- the signature of
/// contention, not any one test being wrong; that observation still
/// holds for the residual, now-isolated contention effect this cap
/// addresses.
///
/// **Response 232: lifted from `runtime::terminal::reader::tests` (where
/// it lived alone, private to that one file) to here**, so
/// `approval::tests::channel`/`approval::tests::reference_adapter` can
/// share the exact same static rather than run under a second,
/// independent pool that would do nothing to bound *cross-module*
/// concurrent forks -- the mechanism response 232 diagnosed for the
/// RFC-021/RFC-022 socket flakes is fd inheritance across `fork()`, not
/// scheduling, so what actually needs bounding is the process-wide
/// count of real spawns in flight at once, not any one file's own.
///
/// **What this removes**: wall-clock overlap between *different test
/// functions'* real processes -- nothing else. It does not remove any
/// coverage of concurrent readers *within* a single test.
pub(crate) struct RealProcessLimiter {
    count: Mutex<usize>,
    freed: Condvar,
}

impl RealProcessLimiter {
    const CAP: usize = 6;

    pub(crate) fn acquire() -> RealProcessSlot {
        static LIMITER: OnceLock<RealProcessLimiter> = OnceLock::new();
        let limiter = LIMITER.get_or_init(|| RealProcessLimiter {
            count: Mutex::new(0),
            freed: Condvar::new(),
        });
        let mut count = limiter
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while *count >= Self::CAP {
            count = limiter
                .freed
                .wait(count)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        *count += 1;
        drop(count);
        RealProcessSlot(limiter)
    }
}

/// RAII permit from [`RealProcessLimiter::acquire`]. Bind this as the
/// *first* local in a test function -- Rust drops locals in reverse
/// declaration order, so binding it first makes it drop *last*, holding
/// the slot for the test's entire real-process lifetime rather than
/// releasing early while cleanup is still in flight.
pub(crate) struct RealProcessSlot(&'static RealProcessLimiter);

impl Drop for RealProcessSlot {
    fn drop(&mut self) {
        let mut count = self
            .0
            .count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *count -= 1;
        drop(count);
        self.0.freed.notify_one();
    }
}
