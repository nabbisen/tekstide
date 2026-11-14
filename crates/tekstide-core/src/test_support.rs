#[cfg(test)]
use std::sync::{Condvar, Mutex, OnceLock};

/// Removes `path` (a directory or a file) when the current test's thread
/// ends, and returns it.
///
/// **How a fixture builder that returns a bare `PathBuf` stops leaking
/// without changing its callers.** Each `#[test]` runs on its own thread, so
/// the thread-local's destructor is the end of the test. Before this the
/// suite left about 43 000 entries in `/tmp` (review 421/422). Not for a
/// path another thread or process must outlive the test's own thread.
///
/// `#[cfg(test)]`, explicitly: review 624 broadened this module's own
/// gate to `#[cfg(any(test, feature = "test-support"))]` so `tekstide`'s
/// consuming tests could reach the new `SHELL_READINESS_*` constants
/// below; every item that was relying on the module's own old, narrower
/// `#[cfg(test)]` needs that cfg restated on itself now, or it compiles
/// (and, being `pub(crate)` and unreachable, lints as dead code) in a
/// `feature = "test-support"`-only build too.
#[cfg(test)]
pub(crate) fn remove_when_this_test_ends(path: std::path::PathBuf) -> std::path::PathBuf {
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            if std::fs::remove_dir_all(&self.0).is_err() {
                let _ = std::fs::remove_file(&self.0);
            }
        }
    }
    thread_local! {
        static SCRATCH: std::cell::RefCell<Vec<Remove>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    SCRATCH.with(|scratch| scratch.borrow_mut().push(Remove(path.clone())));
    path
}

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
#[cfg(test)]
pub(crate) struct RealProcessLimiter {
    count: Mutex<usize>,
    freed: Condvar,
}

#[cfg(test)]
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
#[cfg(test)]
pub(crate) struct RealProcessSlot(&'static RealProcessLimiter);

#[cfg(test)]
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
#[cfg(test)]
pub(crate) struct KillOnDropChild(Option<std::process::Child>);

#[cfg(test)]
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

#[cfg(test)]
impl Drop for KillOnDropChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The macOS load-average diagnostic: a measurement assertion whose message
/// blames load has to report the load it actually measured, or say plainly
/// that it could not read one -- never an empty string and never a
/// plausible-looking zero, which is exactly what `std::fs::read_to_string(
/// "/proc/loadavg")` degraded to on macOS (no `/proc` there) and what put
/// `drawing_the_largest_tree`'s own register row in a four-day
/// misattributed state: every one of its five macOS occurrences printed
/// `("")`, and nobody noticed the instrument itself was the thing broken.
///
/// `getloadavg(3)` rather than `/proc/loadavg` on Linux and something else
/// on macOS: one mechanism that works on both, declared in `libc` for both
/// (`unix/bsd/mod.rs`, which `unix/bsd/apple` shares, and `unix/linux_like`)
/// -- a second mechanism per platform is a second thing that can silently
/// return nothing, the exact failure mode this function exists to stop.
/// The core count comes from `std::thread::available_parallelism`, so a
/// caller's message can state the comparison ("load average well above
/// core count") rather than only assert it, the second defect the
/// `drawing_the_largest_tree` message had alongside its empty string.
///
/// Returns a ready-to-print fragment, never panics, and never silently
/// loses a reading: either both numbers, or an explicit statement of
/// which one (or both) could not be obtained.
pub fn load_average_report() -> String {
    let mut samples: [libc::c_double; 1] = [0.0];
    // SAFETY: `samples` is a valid, correctly-sized buffer for `nelem = 1`,
    // and `getloadavg` writes at most that many elements into it.
    let obtained = unsafe { libc::getloadavg(samples.as_mut_ptr(), 1) };
    let load = (obtained == 1).then_some(samples[0]);
    let cores = std::thread::available_parallelism().map(usize::from).ok();
    match (load, cores) {
        (Some(load), Some(cores)) => {
            format!("load average (1 min): {load:.2}, {cores} core(s)")
        }
        (Some(load), None) => {
            format!("load average (1 min): {load:.2}, core count unavailable")
        }
        (None, Some(cores)) => format!("load average unavailable, {cores} core(s)"),
        (None, None) => "load average and core count both unavailable".to_string(),
    }
}

/// Locates a sibling `[[bin]]` target a test wants to spawn as a real
/// process (`reference_adapter`, `project_process_probe`,
/// `audit_store_create_probe`) -- the one path-derivation rule six
/// independently-maintained copies, across both crates, used to get
/// wrong together the day cargo 1.100.0 moved where a unit test's own
/// binary lives. One copy now, here: `tekstide`'s own test code already
/// reaches this module through the `test-support` feature its
/// `[dev-dependencies]` enables, and this function references no other
/// module (`std` only), so it adds no new entry to the production graph
/// RFC-074 D1's gate measures -- confirmed by re-running that gate
/// after adding this, not assumed.
///
/// `CARGO_BIN_EXE_<name>` is set by cargo only for genuine integration
/// test targets (`tests/*.rs`), never for a lib's own `#[cfg(test)]`
/// unit tests -- which is every caller of this function -- so that fast
/// path is checked first and, when absent, this searches rather than
/// counts levels. Measured directly rather than read from a changelog:
/// a unit test's own binary sits at `target/<profile>/deps/<crate>-
/// <hash>` (so the `[[bin]]` target is two ancestors up, at the profile
/// root) through at least cargo 1.99.x, and at `target/<profile>/
/// build/<crate>-<hash>/out/<crate>-<hash>` (five ancestors up) from
/// cargo 1.100.0 on. Ascending `current_exe()`'s ancestors and testing
/// each for a sibling named `name`, bounded at a handful of levels,
/// resolves correctly under either layout without hardcoding which one
/// is current; the bound keeps a search that will never succeed from
/// walking the whole filesystem to the root.
pub fn spawned_helper_binary_path(name: &str) -> std::path::PathBuf {
    if let Ok(path) = std::env::var(format!("CARGO_BIN_EXE_{name}")) {
        return std::path::PathBuf::from(path);
    }
    let test_exe = std::env::current_exe().expect("current_exe should resolve for a running test");
    const MAX_ANCESTORS: usize = 6;
    let mut searched = Vec::new();
    for ancestor in test_exe.ancestors().skip(1).take(MAX_ANCESTORS) {
        let candidate = ancestor.join(name);
        if candidate.is_file() {
            return candidate;
        }
        searched.push(candidate);
    }
    panic!(
        "could not find the `{name}` [[bin]] target near the running test binary ({}) -- the \
         [[bin]] target may not have built, or cargo has moved the test binary's own location \
         again. Searched:\n{}",
        test_exe.display(),
        searched
            .iter()
            .map(|path| format!("  {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[cfg(test)]
mod tests;
