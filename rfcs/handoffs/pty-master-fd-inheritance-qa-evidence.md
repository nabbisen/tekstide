# QA evidence: pty-master-fd-inheritance

## The defect, reproduced before it was fixed

Automated, not a one-off manual walkthrough: `a_second_terminals_child_inherits_no_descriptor_for_the_first_terminals_pty_master`
(`crates/tekstide-core/src/runtime/terminal/tests.rs`) launches two real terminals through
`LinuxTerminalRuntime::launch_project_shell` and lists the second child's `/proc/<pid>/fd`.

With both parts of the fix (`OpenPty::new`'s `set_cloexec` calls, and `spawn_pty_child`'s
`close_range` belt-and-brace) commented out:

```
thread '...' panicked at crates/tekstide-core/src/runtime/terminal/tests.rs:763:5:
the second terminal's real child must not hold any descriptor for the first terminal's PTY
master -- found 2 such descriptor(s) in /proc/3939882/fd
```

The second terminal's own child held 2 descriptors resolving to `/dev/ptmx` -- the first
terminal's master, exactly the shape `tekstide-finding-pty-master-fds-inherited-by-every-child.md`
measured on a live survivor (27 in that case; 2 here because only one other terminal was open at
the time, not dozens).

## After: no inherited descriptor at all

Both parts restored, same test: `0` `/dev/ptmx` entries in the second child's fd table. Green.

## Ablated, twice -- each layer proven independently, not just "both together happen to work"

1. **Both disabled** (`set_cloexec` calls commented out, `close_range` commented out): test fails,
   2 inherited masters found. Reproduces the defect.
2. **`set_cloexec` restored, `close_range` still disabled**: test passes. Item 1 alone closes the
   specific hole this document measured.
3. **`set_cloexec` disabled again, `close_range` restored**: test passes. Item 3 alone also closes
   it -- not decorative, a real independent layer, verified the same way item 1 was, not asserted
   from reading the syscall's manual page.
4. Both restored. Green. No `TEMP ABLATION` markers left in the tree.

## Every descriptor site this runtime opens

Enumerated by grepping `runtime/terminal/*.rs` for every fd-creating call
(`openpty`, `dup`, `eventfd`, `File::from_raw_fd`, `.open(`), not only the master this document
names:

| Site | What it is | CLOEXEC? | Action |
| --- | --- | --- | --- |
| `pty.rs`, `openpty()` master | The PTY master, held for the terminal's whole life | Was **not** set | **Fixed** -- `set_cloexec` |
| `pty.rs`, `openpty()` slave | The original slave fd, alive only from open to `close_slave()` | Was **not** set | **Fixed** -- `set_cloexec`. Does not affect `duplicate_slave`'s own dups (below); `dup(2)` never copies `FD_CLOEXEC` onto a new fd regardless of the source's flag |
| `pty.rs`, `duplicate_slave` (`libc::dup`) x4 per spawn | stdin/stdout/stderr/ctty for the child being spawned | Correctly **not** set, by design | No change -- these four *must* survive exec, that is their entire purpose. `dup(2)`'s own semantics already guarantee this without any code here asking for it |
| `reader.rs`, `create_eventfd` (shutdown, wake) x2 | Reader-thread wake/shutdown signalling | Already `EFD_CLOEXEC` | No change needed |
| `reader.rs`, `try_clone_wake_notifier` (`File::try_clone`) | A duplicate handle onto the wake `eventfd`, for a caller that wants to wait instead of poll | Already CLOEXEC -- Rust's `std::fs::File::try_clone` uses `F_DUPFD_CLOEXEC` internally, not a bare `dup(2)` | No change needed |

Outside `runtime/terminal` but checked for the same shape while this was open, since the finding's
own severity argument (a capability an AI CLI agent could reach) is exactly what this project takes
most seriously:

| Site | CLOEXEC? | Action |
| --- | --- | --- |
| `transcript/writer.rs`, `OpenOptions::open` (transcript file) | Rust's `std::fs::OpenOptions` sets `O_CLOEXEC` by default on Unix | No change needed |
| `approval/channel.rs`, `open_dir_no_follow`/`openat_dir_no_follow` (raw `libc::open`/`openat`) | Already pass `O_CLOEXEC` explicitly in the flags | No change needed |
| `approval/channel.rs`, `UnixListener::bind`/`UnixStream::connect` | Rust's `std::os::unix::net` types set `O_CLOEXEC` by default on Unix, the same as `File` | No change needed |

**Conclusion: the PTY master and the PTY slave were the only two descriptors in this codebase
without close-on-exec.** Everything else already had it, most of it by relying on a Rust standard
library guarantee rather than an explicit flag -- which is also why this single omission stood out
enough to be found: it is the one place raw `libc::openpty` was used without the same discipline
every other fd-opening call site in this codebase already had.

## Measured: per-run leak, before and after

Isolated to the one test already known to leak (`test-process-leak.md`'s own "~28/run from one
benchmark test" figure, and RFC-043's own README): `terminal_session_limit_headless_n_pane_wake_throughput_benchmark`,
which launches `1+3+6+8+10 = 28` panes, each running the backgrounded `FLOOD_SCRIPT` loop.
Measured against a clean baseline (`shells=0 pts=4` before each run), not the developer machine's
already-elevated count from earlier work this session.

| | Immediately after the run | +36s (past `FLOOD_SCRIPT`'s own 30s internal deadline) |
| --- | --- | --- |
| **Before this fix** | 28 leaked shells, `pts` 4 → 32 | **Still 28**, `pts` still 32 -- confirmed *not* self-terminating, matching the finding's "blocked, not spinning, hours old" observation directly rather than trusting it on report |
| **After this fix** | 28 shells present, but `pts` stayed at **4** -- not 32 | Down to **2**, both later traced to an unrelated leftover from a different test run started during the wait window (`sh` spawning `sleep 1` repeatedly -- no `FLOOD_SCRIPT` content, not one of the 28), not the FLOOD loops themselves; killed separately. All 28 of the actual benchmark's own loops were gone by this check |

`pts` staying at 4 immediately after the run, not merely recovering faster afterward, is the
cleanest single number here -- it needs no waiting and no risk of conflating this measurement with
something else running concurrently on the same machine, which is exactly what happened to the
process-count column above (kept for completeness, with the contamination stated rather than
quietly excluded from the count).

**The mechanism changed exactly as the finding predicted, and this is the second, independent
confirmation of it (the first being the fd-inheritance test above).** Before the fix, the 28
orphaned loops were `state=S`, `wchan=wait_woken` -- blocked forever on a `printf` into a PTY whose
master something else still held open. After the fix, the same 28 were `state=R<` -- running, not
blocked, because the master genuinely has no other holder once this terminal's own owner closes
it, so the write gets `EIO` instead of hanging, and the loop reaches its own 30-second exit check
on the next iteration instead of never reaching it at all.

**`/dev/pts` no longer accumulates at all**, not merely faster: the PTY slot frees the moment this
terminal's own master closes, regardless of whether the orphaned job inside it has actually exited
yet -- because nothing else in the process still holds a copy of that master to keep the slot
allocated. This is the leak's *self-sustaining* mechanism (Consequence 2 in the finding) closed,
independent of RFC-043's still-open job-group-signalling gap (Consequence 2's "each survivor keeps
the next one alive" no longer applies once masters cannot be inherited).

**What this fix does not change**: the 28 backgrounded loops still orphan transiently (RFC-043's
own job, not this one) -- they now merely stop holding a PTY slot and stop running indefinitely
while doing so, rather than not existing at all. RFC-043's own measurement should be taken after
this fix (as its own README already says), since some of what used to look like "the leak" was
this mechanism, not the missing session-signal step.

## Gates

`cargo build --workspace --all-targets`, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --all-features -- -D warnings`, `git diff --check`: all
clean. `runtime::terminal::` module: 67 of 67 tests pass, including the new one.

Full-workspace three-run gate: see review request for the exact counts (taken after this document
was written, so this file states methodology rather than a number that could go stale if the gate
is re-run later).
