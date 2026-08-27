---
title: "The suite assumes it owns the machine — QA evidence"
rfc: "none"
source_rfc_status: "No RFC. Implementation evidence for suite-assumes-it-owns-the-machine.md."
target_milestone: "M12"
created: "2026-08-27"
---

# Evidence

## Item 1 — `transcripts/`/`approval/` isolation

### Enumeration, per the handoff's own required step

Every site in production code that resolves `$XDG_STATE_HOME`/`$HOME/.local/state/tekstide`
(`AppStatePathProvider::linux_default()` and its callers), found by grepping for
`linux_default`/`XDG_STATE_HOME`/`AppStatePathProvider` across both crates:

| Site | Resolves | Reachable from a test? | Isolated? |
| --- | --- | --- | --- |
| `AppStatePathProvider::linux_default()` (`tekstide-core/src/project/recent/store.rs`) | the raw path itself | N/A — pure resolver, isolation belongs at its callers | N/A |
| `ConfigPathProvider::linux_default()` (`tekstide-core/src/config/path.rs`) | `$XDG_CONFIG_HOME`/`$HOME/.config/tekstide` | **No production caller anywhere in either crate.** Confirmed by `grep -rn "ConfigPathProvider::linux_default"` returning zero call sites at all. Dead in production; only `linux_from_env` (its injectable, always-explicit sibling) is exercised, by tests. | **No change needed** — nothing calls the unguarded form, in a test or otherwise. |
| `main.rs`'s `boot()` | recent-projects store, via `AppStatePathProvider::linux_default()` | **No.** `boot()` is `iced`'s own `BootFn`, called only from `main()`. Confirmed: `grep -rn "boot("` across the crate shows it referenced only inside `main.rs` itself and in doc comments; every test that reaches equivalent logic goes through `open_cli_project_path_and_record` instead (`tests.rs`'s own doc: "the real logic `boot()`'s CLI-argument loop reaches"). | **No change needed** — unreachable from any test today. Left as-is rather than isolated speculatively; isolating a path nothing can reach would be the same mistake in reverse (code with no test to prove it). |
| `resolve_audit_state_dir` (`shell.rs`) | `audit/`, via the same `linux_default()` | Yes | **Already isolated** — `audit-store-test-isolation.md`'s own fix, unchanged here. |
| `open_real_agent_run_state_root` (`shell.rs`) | `transcripts/`, and (via the same root) `approval/` | Yes — `attempt_agent_run_launch_with_profile`'s plain, two-argument form, reached from ~9 test sites in `shell/tests.rs` | **Fixed by this slice.** |

**The `approval/` subtree is not a second site.** `ApprovalChannelPathRequest`/
`ApprovalChannelPathResolver::resolve` (`tekstide-core/src/approval/channel.rs`) take an
already-resolved `state_root: PathBuf` as a parameter and never read `XDG_STATE_HOME`/`HOME`
themselves (confirmed: no `linux_default`/env-var read anywhere in that file). The `state_root`
they receive traces back through `AgentRunLaunchRequest.approval_state_root` to the identical
`open_real_agent_run_state_root()` call the transcript path uses — "a `Managed` launch binds its
socket under the state root" is the same resolution, not a third one.

### The fix

`open_real_agent_run_state_root` gets the identical shape `resolve_audit_state_dir` already has —
reused, not reinvented, per the handoff's own instruction:

- The function's own body is now `resolve_agent_run_state_dir()?` plus `create_dir_all`, unchanged
  from what it did before; the isolation seam moved into a new `resolve_agent_run_state_dir`,
  `#[cfg(not(test))]`/`#[cfg(test)]`-split exactly as `resolve_audit_state_dir` is.
- The `#[cfg(test)]` branch: a thread-local, lazily created on first use per thread, memoized for
  the rest of that test — no per-call-site wiring, so the ~9 known call sites and any test written
  after this comment are both safe automatically.
- `assert_not_the_real_agent_run_state_dir`: belt-and-suspenders, checks the resolved directory
  against the real one and panics naming both paths if they ever coincide.
- **No named-override hook** (unlike `resolve_audit_state_dir`'s own `test_audit_state_dir`).
  Checked rather than assumed: none of the ~9 tests reaching this path inspect
  `transcripts/`/`approval/` contents through it — they check in-memory launch outcomes (pane
  count, refusal reason). The one class of test that *does* need to inspect a transcript
  (`agent_run_transcript_window`'s own tests) already bypasses this function entirely via an
  explicit `state_root` parameter on `agent_run_transcript_window_with_state_root`. Add a named
  hook if that stops being true; not added speculatively.

**One real difference from the audit case, found the hard way.** The audit fix's own directory
naming (`tekstide-audit-test-default-<pid>-<sequence>-<ThreadId debug repr>`) is long, and that
never mattered for the audit store — a SQLite file path has no meaningful length limit here. It
mattered immediately for this fix: an approval socket binds *under* this directory
(`ApprovalChannelDirectory::socket_path`, `<state_root>/approval/<agent run id>.sock`), and
`sockaddr_un`'s `sun_path` has real, small capacity (`max_socket_path_len`, ~107 bytes). The first
version of this fix, using the audit scheme verbatim, made every test reaching a real `Managed`
launch fail with `SocketPathTooLong` the moment it went live (21 failures, all at the same call
site). Fixed with a much shorter name (`tekstide-run-<pid>-<sequence>`, dropping the thread-id
entirely — the global, monotonic counter already guarantees a distinct directory per call
regardless of which thread makes it, so the thread id added no uniqueness this scheme did not
already have). Full socket path now measured at ~82 bytes worst case against real UUID-length
agent-run ids, comfortably inside the ~107-byte limit.

### Reproduced first, per the handoff's own requirement

Snapshotted `~/.local/state/tekstide` before touching anything:

```
transcripts/ file count: 14573    transcripts/ dir count: 27334
approval/ entries: 1              audit/ entries: 1
```

Reverted the fix (`git stash` on `shell.rs` alone), ran `env -u XDG_STATE_HOME cargo test -p
tekstide --bin tekstide` (449/449 passed — the defect does not fail tests, it silently pollutes a
real directory), then re-measured:

```
transcripts/ file count: 14599 (+26)    transcripts/ dir count: 27383 (+49)
approval/ entries: 1                    audit/ entries: 1
newest transcripts/ mtime: matches the run just completed
```

`transcripts/` moved; `approval/`/`audit/` did not (no test in this run happened to reach a
`Managed` adapter launch that binds a socket, but the same call reaches the same unguarded
resolver either way — the socket-length failure surfaced the moment the *fixed* code's own
default was too long, confirming the approval path is live too).

### After: the diff is empty

Restored the fix, rebuilt, ran the identical command again:

```
transcripts/ file count: 14599 (unchanged)    transcripts/ dir count: 27383 (unchanged)
approval/ entries: 1 (unchanged)              audit/ entries: 1 (unchanged)
newest transcripts/ mtime: unchanged -- still the pre-fix-run file, no new write
```

Not "audit is clean" — empty, across every subtree this slice owns.

### The guard, ablated

Temporarily replaced `resolve_agent_run_state_dir`'s `#[cfg(test)]` body with a direct call to
`AppStatePathProvider::linux_default()` (bypassing the thread-local entirely) and re-ran a test
reaching the guarded path:

```
thread '...' panicked at crates/tekstide/src/shell.rs:2876:9:
assertion `left != right` failed: open_real_agent_run_state_root resolved the real,
developer-owned state directory ("/home/nabbisen/.local/state/tekstide") inside a test build.
See rfcs/handoffs/suite-assumes-it-owns-the-machine.md.
```

Restored; the same test passes again.

### Gate

`fmt`, `clippy -D warnings`, `git diff --check`: clean. Three consecutive full-workspace runs:
449 + 746 + 2 clean, `/dev/pts` flat at 13 before and after (this slice touches only a state-root
resolution, not the terminal runtime — no PTY-occupancy change expected or observed).

**One transient, unrelated failure noted, not chased**: run 2 of the three hit
`transcript::reader::tests::a_window_starting_inside_a_real_control_sequence_classifies_identically_to_the_whole`
and `..._ablation_without_resynchronization_the_split_misclassifies` in `tekstide-core` — a crate
this slice's own code (`crates/tekstide`) cannot reach at all, structurally, the same reasoning
`audit-store-test-isolation.md` used to rule out rows 1/2/5 of the flake register. Both use
`capture_real_sgr_output`, a real-process-spawning helper, and both passed cleanly across five
immediate re-runs of the full `tekstide-core` lib suite afterward — consistent with ordinary
scheduling noise, not a regression this slice introduced. Not previously recorded in
`test-process-leak.md`; disclosed here rather than chased further, out of this handoff's own
scope.

## Item 2 — the wall-clock assertion

`change_review_content_view_build_cost_by_line_count_measurement`'s message no longer claims which
cause a failure indicates. Prints the 1-minute load average (`/proc/loadavg`) alongside the figure
on failure — verified directly by temporarily lowering the budget to force a failure:

```
view-build cost at 100 lines exceeded the 500ms budget (NFR-PERF-003's own is 16ms p95) -- got
0ms. This is either a real regression or this machine was under load when it ran; load average
(1min, from /proc/loadavg): 2.85. A number well above the core count points at load, not a
regression -- rerun on an idle machine before treating this as one.
```

Budget unchanged (500ms) — the review 338 failures happened at load 59.7, far past what raising a
500ms bound to survive would mean tolerating; the reasoning is recorded in the test's own doc
comment, not "it kept failing."

**`#[ignore]` question, answered: no.** Checked for precedent first, per the handoff's own claim
that one exists "either way" — found none: `grep -rn "#\[ignore" crates/` returns zero matches
anywhere in this codebase. The real precedent is the flake register
(`test-process-leak.md`) and the sibling diagnostic benchmark
(`real_repository_filesystem_scan_cost_headless_benchmark`, `tekstide-core`, which asserts nothing
at all and only reports). Decided to keep the assertion (the handoff's own acceptance wants a
budget, changed or not, not removed) and record the test in the flake register instead of
`#[ignore]`-ing it — a recorded, honestly-worded flake still runs and still catches a real
regression the day one lands; an ignored test stops running at all. Added as row 7 in
`test-process-leak.md`, with a note distinguishing its cause (CPU load) from every other row's
(process-leak/audit-store pressure).

### Gate

`fmt`, `clippy -D warnings`: clean. Test passes at rest (six line counts, all comfortably under
budget on this idle machine).
