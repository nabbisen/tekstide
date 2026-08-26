---
title: "RFC-043: QA evidence"
rfc: "RFC-043"
rfc_file: "../../accepted/043-terminal-process-containment.md"
source_rfc_status: "Accepted 2026-08-26 — M12"
target_milestone: "M12"
created: "2026-08-26"
---

# QA evidence

## PR-043-A — make the leak red

### The guard, and where it had to live

`RunningTerminal::drop` (`crates/tekstide-core/src/runtime/terminal/launch.rs`), after the
existing `SIGKILL`-the-process-group-and-wait sequence: enumerates every live process whose
`/proc/<pid>/stat` session field matches `self.process_group_id` (which is also this terminal's
session id -- `spawn_pty_child`'s `pre_exec` calls `setsid()` before `exec`, making the freshly
`fork`ed shell both its own process group leader and its own session leader at launch) and panics,
naming every survivor, if the session is not empty.

**Not wired per test, not opt-in** -- placed in the destructor itself, the same "put it where the
process is created" instruction `what-containment-must-not-become.md` §5 states directly, citing
the audit-store slice's own per-site mistake as the thing not to repeat.

### A real cross-crate gap found and fixed before the guard could actually cover anything

First attempt gated the guard with `#[cfg(test)]`. It fired correctly under `cargo test -p
tekstide-core` (caught the case below immediately), and was **silently absent** -- not skipped,
not disabled, the code did not exist in that build at all -- under `cargo test -p tekstide`, where
almost every real terminal-launching test actually lives. `#[cfg(test)]` inside a library crate
only activates when *that crate's own* test suite is what's compiling; it cannot see across a
dependency edge into a consuming crate's test mode. The benchmark already known to leak 28
processes (`test-process-leak.md`'s own "~28/run" figure) ran clean under this first version --
confirmed directly, not assumed, before reporting it fixed.

**Fixed with a Cargo feature, not a workaround.** `tekstide-core/Cargo.toml` gained a
`test-support` feature (empty, gates nothing on its own); the guard's own `#[cfg]` became
`#[cfg(any(test, feature = "test-support"))]`; `tekstide/Cargo.toml`'s `[dev-dependencies]` now
also depends on `tekstide-core` with that feature enabled, on top of the ordinary
`[dependencies]` entry -- Cargo unifies the two into one build for `cargo test -p tekstide`.
Verified this does **not** leak into a release build: `cargo tree -e features` shows
`test-support` only under the dev-dependency edge, and `strings` on a plain `cargo build -p
tekstide --bin tekstide` binary (no `--tests`) contains zero occurrences of the feature name.

### Ablated

Temporarily commented out the `#[cfg(...)]` attribute and the guard call in `RunningTerminal::drop`
(both lines, so nothing partially applies). Re-ran
`linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm` (below) alone:
**passed**, silently, despite the real backgrounded descendant surviving exactly as it does today.
Restored both lines, re-ran: failed again, same message. No `TEMP ABLATION` markers left.

### The inventory

Deliverable this PR-A slice exists to produce, per its own gate item: "the count of tests that
fail once the guard is live, and which ones. Nobody has one today." Ran each crate's suite three
times; the same four tests failed every time, no others:

| Test | Crate | Why it leaks |
| --- | --- | --- |
| `runtime::terminal::tests::linux_runtime_does_not_overclaim_when_child_outlives_direct_shell_after_sigterm` | `tekstide-core` | Deliberately backgrounds a `trap '' TERM` descendant to prove `request_terminate` reports `KilledAfterTimeout` honestly rather than overclaiming success -- the test's own scenario *is* the escape this RFC exists to close |
| `shell::tests::terminal_session_limit_headless_n_pane_wake_throughput_benchmark` | `tekstide` | Launches `1+3+6+8+10=28` panes running the backgrounded `FLOOD_SCRIPT` loop |
| `shell::tests::terminal_poll_handler_cost_under_a_real_wake_driven_flood_headless_benchmark` | `tekstide` | Same `FLOOD_SCRIPT`, one pane |
| `shell::tests::closing_a_project_with_a_backgrounded_descendant_still_records_applied_while_it_survives` | `tekstide` | Its own name states the scenario -- a real, deliberately backgrounded descendant that is expected to survive a close today; this is D3/D4's own audit-honesty test, and its fixture is exactly what PR-043-B/C's containment has to change the outcome of |

**No other test in either crate's suite went red** across three runs each (742/742 and 441/441
elsewhere, stable). One pre-existing, already-documented flake
(`approval::tests::channel::bind_recovers_from_a_stale_socket_file`, `test-process-leak.md`'s own
row 1, "the original, response 213") appeared once in a `--no-fail-fast` full run and passed
cleanly on three immediate isolated re-runs -- unrelated to this guard (a socket bind error, not a
process/session one), not counted in the inventory above.

### Gate

`cargo build --workspace --all-targets`, `cargo fmt --all -- --check`, `cargo clippy --workspace
--all-targets --all-features -- -D warnings`, `git diff --check`: all clean.

**The suite is not green, by design.** `cargo test --workspace`: 441/444 (`tekstide`), 741/743
(`tekstide-core`), 2/2 (`rfc_docs_invariants`) -- the 4-test inventory above is the only thing red,
stable across repeated runs. Per this slice's own gate item: "the suite will not be green at the
end of this slice, by design. Say so plainly; do not skip the guard's rollout to keep a green
run." Said plainly here.

## PR-043-B, PR-043-C

Not started.
