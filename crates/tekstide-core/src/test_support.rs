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

/// Kills and reaps the wrapped child on drop -- `std::process::Child::drop`'s
/// own documented behaviour does neither, so a test that panics before its
/// own cleanup leaks the real process. `rfcs/handoffs/test-process-leak.md`:
/// the cause was found 2026-08-16 and, before this fix, had already
/// produced three distinct, separately-disclosed intermittent failures
/// under the resulting fork-pressure (`RealProcessLimiter`'s own doc above
/// explains why leaked processes matter beyond tidiness: `fork()`'s cost
/// scales with the parent's thread/fd count). Every real process
/// `approval::tests::channel`/`approval::tests::reference_adapter` spawn is
/// returned wrapped in this type instead of a bare `std::process::Child`.
///
/// `kill`/`wait` proxy directly to the wrapped `Child` and take `&mut
/// self`, matching `Child`'s own signatures for those two methods --
/// existing call sites that already kill and reap a child manually before
/// this fix (`reference_adapter.rs`'s
/// `deciding_a_proposal_whose_real_adapter_process_has_already_exited_is_undeliverable`)
/// needed no change beyond the return type. `wait_with_output` takes `self`
/// by value, matching `Child::wait_with_output`'s own signature, and is the
/// one place the inner `Child` is genuinely taken rather than borrowed --
/// required because a type implementing `Drop` cannot have a field moved
/// out of it directly, so the field is `Option`-wrapped and `.take()`n
/// instead.
///
/// **Drop calling `kill`/`wait` again after a caller already did so
/// manually is intentional, not an oversight**: both are `Result`-returning
/// and their errors are discarded here (`let _ =`), so a redundant
/// kill/wait on an already-reaped process is silently harmless on this
/// platform (`ESRCH`, ignored) -- simpler than tracking "already cleaned
/// up" state to skip it, and correctness does not depend on which caller
/// (this `Drop` impl, or a test's own explicit call) cleans up first.
pub(crate) struct KillOnDropChild(Option<std::process::Child>);

impl KillOnDropChild {
    pub(crate) fn new(child: std::process::Child) -> Self {
        Self(Some(child))
    }

    pub(crate) fn kill(&mut self) -> std::io::Result<()> {
        self.0
            .as_mut()
            .expect("child already consumed by wait_with_output")
            .kill()
    }

    pub(crate) fn wait(&mut self) -> std::io::Result<std::process::ExitStatus> {
        self.0
            .as_mut()
            .expect("child already consumed by wait_with_output")
            .wait()
    }

    pub(crate) fn wait_with_output(mut self) -> std::io::Result<std::process::Output> {
        self.0
            .take()
            .expect("child already consumed by wait_with_output")
            .wait_with_output()
    }
}

impl Drop for KillOnDropChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Test-only process-liveness check, mirroring
/// `runtime::terminal::termination::process_group_exists_by_id`'s own
/// `libc::kill(.., 0)` technique -- sending signal `0` sends nothing; the
/// kernel only validates that the target exists and is signalable, which is
/// exactly "is this pid still in the process table" with no side effect.
/// `ESRCH` (no such process) is the only outcome this needs to distinguish
/// from "still there" -- ambient in tests, so any other error (e.g.
/// `EPERM`, which would mean the process exists but this test cannot
/// signal it) is treated as "still alive" rather than modelled separately,
/// unlike the production runtime code this mirrors.
#[cfg(test)]
pub(crate) fn process_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

#[cfg(test)]
mod tests {
    use super::{KillOnDropChild, RealProcessLimiter, process_is_alive};

    fn spawn_real_sleep() -> std::process::Child {
        std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .expect("spawn a real, short-lived sleep process")
    }

    /// **Shows the leak happening** -- the defect `KillOnDropChild` fixes,
    /// reproduced directly rather than only cited: a bare `Child`, moved
    /// into a closure that panics before it would otherwise be cleaned up,
    /// leaves the real process running after the panic unwinds past it.
    /// `catch_unwind` contains the panic so this test itself does not fail;
    /// the leaked process is killed manually afterward specifically because
    /// this test's whole point is that nothing else would have.
    #[test]
    fn a_bare_child_leaks_across_a_panic_this_fix_exists_to_prevent() {
        // First local, per RealProcessLimiter::acquire's own doc: every
        // real-process test in this crate bounds its concurrent forking
        // through this limiter, this module's own new tests included.
        let _real_process_slot = RealProcessLimiter::acquire();
        let child = spawn_real_sleep();
        let pid = child.id();

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _child = child;
            panic!("deliberate panic, before this closure's own cleanup would run");
        }));
        assert!(
            panicked.is_err(),
            "test precondition: the closure must actually have panicked"
        );

        assert!(
            process_is_alive(pid),
            "this demonstrates the real defect: a bare Child leaks across a panic, per \
             Child::drop's own documented behaviour"
        );

        // Manual cleanup: this test intentionally reproduces the leak, so
        // nothing else in the process will reap it.
        let _ = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        let _ = unsafe { libc::waitpid(pid as libc::pid_t, std::ptr::null_mut(), 0) };
    }

    /// **Shows the leak not happening** -- the same scenario, the same
    /// panic, `KillOnDropChild` in place of the bare `Child`. The process
    /// must be gone (killed *and* reaped, not merely signalled) by the time
    /// `catch_unwind` returns, since `Drop::drop` runs synchronously during
    /// unwinding, before `catch_unwind` regains control.
    #[test]
    fn kill_on_drop_child_does_not_leak_across_a_panic() {
        let _real_process_slot = RealProcessLimiter::acquire();
        let child = spawn_real_sleep();
        let pid = child.id();
        let guard = KillOnDropChild::new(child);

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = guard;
            panic!("deliberate panic, before this closure's own cleanup would run");
        }));
        assert!(
            panicked.is_err(),
            "test precondition: the closure must actually have panicked"
        );

        assert!(
            !process_is_alive(pid),
            "KillOnDropChild must have killed and reaped the real process during unwind, before \
             catch_unwind returned control here"
        );
    }

    /// The non-panicking path still leaves nothing running -- proven
    /// separately from the panic case above, since a `Drop` impl that only
    /// works when reached in the "normal" order is not actually proven by a
    /// test that only exercises the panic order.
    #[test]
    fn kill_on_drop_child_cleans_up_on_ordinary_drop_too() {
        let _real_process_slot = RealProcessLimiter::acquire();
        let child = spawn_real_sleep();
        let pid = child.id();
        {
            let _guard = KillOnDropChild::new(child);
        }
        assert!(
            !process_is_alive(pid),
            "dropping the guard normally (no panic) must still kill and reap the real process"
        );
    }

    /// `wait_with_output` must not double-kill/-wait through `Drop` in any
    /// way that turns success into a failure -- the real exit status and
    /// stdout must still come back correctly after the value has passed
    /// through the guard.
    #[test]
    fn wait_with_output_returns_the_real_exit_status() {
        let _real_process_slot = RealProcessLimiter::acquire();
        let child = std::process::Command::new("sh")
            .arg("-c")
            .arg("echo real-output && exit 0")
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("spawn a real shell command");
        let guard = KillOnDropChild::new(child);

        let output = guard
            .wait_with_output()
            .expect("wait_with_output should succeed for a real, exited process");

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "real-output"
        );
    }
}
