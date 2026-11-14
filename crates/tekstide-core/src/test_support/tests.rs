/// The real `getloadavg(3)` on this machine, not a mock -- proves the
/// call succeeds and the report carries an actual number, at least on
/// whichever platform this test runs on. Does not and cannot prove the
/// macOS branch returns real data from here; that is what the
/// deliberately-forced macOS run (quoted verbatim in the review
/// request) is for.
#[test]
fn load_average_report_carries_a_real_reading_on_this_machine() {
    let report = super::load_average_report();
    eprintln!("load_average_report(): {report}");
    assert!(
        !report.is_empty() && !report.contains("(\"\")"),
        "must never be empty or the old broken format's own empty string: {report:?}"
    );
    assert!(
        report.contains("core(s)"),
        "the core count must always be stated when available (and \
         std::thread::available_parallelism() succeeds on every platform this suite \
         targets), not silently dropped alongside a missing load reading: {report:?}"
    );
}

/// Proves the fast path and the search both actually reach a real,
/// already-built `[[bin]]` target -- not merely that the function
/// returns without panicking. `CARGO_BIN_EXE_reference_adapter` is
/// set here (this crate's own test binary is a unit test, so cargo
/// does not set it on its own); unset, the fallback search must
/// still find the identical file by walking `current_exe()`'s
/// ancestors, on whichever cargo version is actually running this.
#[test]
fn spawned_helper_binary_path_finds_the_real_reference_adapter_both_ways() {
    let via_env = std::env::var("CARGO_BIN_EXE_reference_adapter")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| super::spawned_helper_binary_path("reference_adapter"));
    assert!(
        via_env.is_file(),
        "reference_adapter must already be built for this test to mean anything: {via_env:?}"
    );

    // SAFETY: this test does not spawn threads of its own, and no other
    // test in this process reads or writes this exact variable.
    let saved = std::env::var_os("CARGO_BIN_EXE_reference_adapter");
    unsafe {
        std::env::remove_var("CARGO_BIN_EXE_reference_adapter");
    }
    let via_search =
        std::panic::catch_unwind(|| super::spawned_helper_binary_path("reference_adapter"));
    // SAFETY: restoring exactly what was removed above, before any
    // assertion below can leave it unset for a later test.
    unsafe {
        if let Some(value) = saved {
            std::env::set_var("CARGO_BIN_EXE_reference_adapter", value);
        }
    }

    let via_search = via_search.expect("the ancestor search must find the built binary");
    assert!(via_search.is_file());
    assert_eq!(
        std::fs::canonicalize(&via_search).unwrap(),
        std::fs::canonicalize(&via_env).unwrap(),
        "the fast path and the ancestor search must resolve to the same real file"
    );
}

/// **The search shown failing honestly, not only shown succeeding.**
/// A name that can never exist as a sibling at any ancestor must
/// panic with a message naming every directory it actually tried --
/// the property that matters here, since the previous, single-guess
/// version named one path that was never going to exist the moment
/// cargo changed layout, and nothing about *that* failure was
/// diagnosable without already knowing the answer.
#[test]
fn spawned_helper_binary_path_names_every_directory_it_searched_on_failure() {
    let name = "tekstide-test-support-binary-that-does-not-exist-anywhere";
    let result = std::panic::catch_unwind(|| super::spawned_helper_binary_path(name));
    let error = result.expect_err("a name with no matching binary anywhere must panic");
    let message = error
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| error.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .expect("panic payload should be a string");
    assert!(
        message.contains(name),
        "the panic message must name what it was looking for: {message}"
    );
    assert!(
        message.contains("Searched:") && message.lines().count() > 1,
        "the panic message must list the directories actually searched, not a single \
         guessed path: {message}"
    );
}

/// Review 422: a builder that hands its path to
/// `remove_when_this_test_ends` leaves nothing behind once the test's
/// thread ends -- for a directory with contents, and for a bare file.
#[test]
fn a_path_handed_to_the_test_scratch_is_gone_when_the_thread_ends() {
    let (dir, file) = std::thread::spawn(|| {
        let dir = std::env::temp_dir().join(format!("ts-scratch-dir-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("inner")).unwrap();
        std::fs::write(dir.join("inner/f"), b"x").unwrap();
        let file = std::env::temp_dir().join(format!("ts-scratch-file-{}", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        super::remove_when_this_test_ends(dir.clone());
        super::remove_when_this_test_ends(file.clone());
        assert!(
            dir.is_dir() && file.is_file(),
            "both exist while the test runs"
        );
        (dir, file)
    })
    .join()
    .unwrap();
    assert!(!dir.exists(), "directory left behind: {dir:?}");
    assert!(!file.exists(), "file left behind: {file:?}");
}

use super::{KillOnDropChild, RealProcessLimiter};
use crate::runtime::terminal::process_is_alive;

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

/// Review 638's own deterministic proof, local, no runner needed:
/// `kill(pid, 0)` succeeds against an **unreaped zombie** -- the pid
/// table entry still exists; the kernel simply discards the signal
/// rather than delivering it to anything. This is a statement about
/// POSIX, not about any one test, which is why it is provable here
/// instead of waiting for `runtime::terminal::tests::
/// a_real_backgrounded_job_is_dead_after_a_real_close`'s own
/// intermittent macOS failure to demonstrate it.
///
/// **Review 640's own required correction: proves both halves, not
/// only the kernel fact.** The first version of this test stopped at
/// "`process_is_alive` reads `true` for a zombie" -- true at the
/// time, because [`process_is_alive`] was a bare `kill(pid, 0)`. That
/// shape invited exactly the bug review 640 found at fourteen call
/// sites: every caller had to remember to also exclude
/// `is_gone_or_going` itself, and most did not. [`process_is_alive`]
/// is now the honest answer on its own (`kill(pid, 0) == 0 &&
/// !is_gone_or_going(pid)`), so this test now proves *both* the raw
/// kernel fact (`kill` alone still succeeds -- that part of POSIX
/// has not changed) *and* that the now-corrected helper is immune to
/// it (`process_is_alive` itself reads `false`). The second
/// assertion is what actually matters to every caller; the first is
/// what makes the helper's own body something other than a bare
/// `kill` worth having.
///
/// Deliberately does not reap the child through [`KillOnDropChild`]
/// or any other automatic path -- staying unreaped is the entire
/// point under test -- so this test reaps it manually at the end,
/// the same shape
/// `a_bare_child_leaks_across_a_panic_this_fix_exists_to_prevent`
/// above already uses for its own deliberately-unreaped child.
/// `waitid(..., WNOWAIT)` confirms the child has actually finished
/// exiting, without reaping it, before the real assertion runs --
/// otherwise a slow kernel could let this test observe "signalled,
/// not yet dead" rather than the zombie state the hypothesis is
/// about.
#[test]
fn kill_signal_zero_succeeds_against_an_unreaped_zombie() {
    let _real_process_slot = RealProcessLimiter::acquire();
    let mut child = spawn_real_sleep();
    let pid = child.id() as libc::pid_t;

    // SAFETY: `pid` is this test's own just-spawned child; `SIGKILL`
    // delivers a real, fatal signal to it.
    let kill_result = unsafe { libc::kill(pid, libc::SIGKILL) };
    assert_eq!(
        kill_result, 0,
        "test precondition: SIGKILL must be delivered to this test's own child"
    );

    // SAFETY: `info` is a plain, fully-zeroed `siginfo_t` the call
    // writes into; `WNOWAIT` is the property this call exists for --
    // it reports the state change without reaping, which is exactly
    // what distinguishes "confirmed exited" from "confirmed exited
    // *and reaped*" below.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let waitid_result = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &raw mut info,
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    assert_eq!(
        waitid_result, 0,
        "test precondition: the child must be confirmed to have actually exited (via the \
         SIGKILL above), observed without reaping it"
    );

    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        0,
        "kill(pid, 0) must still succeed against this unreaped zombie -- the raw POSIX fact \
         that made process_is_alive unsafe to use bare, review 638"
    );
    assert!(
        !process_is_alive(pid as u32),
        "process_is_alive itself must read false for this unreaped zombie -- review 640's \
         own correction, excluding on is_gone_or_going so every caller is covered without \
         needing to remember to"
    );

    // Manual cleanup through the `Child` API itself, not another raw
    // `libc` call: deliberately unreaped until now, so nothing else
    // in the process will reap it, and this is also what satisfies
    // `clippy::zombie_processes`'s own static check that every
    // spawned `Child` is eventually `wait()`ed on.
    let _ = child.wait();
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
