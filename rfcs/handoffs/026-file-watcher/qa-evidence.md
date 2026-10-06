# RFC-026 — QA evidence

## PR-026-A — the batching, proved against a simulated stream

**No dependency added, no watcher wired, no surface changed.** The slice adds `crates/tekstide-core/src/project/watch.rs` (the batcher), its tests in `project/watch/tests.rs`, and two `pub` lines in `project.rs`.

### What was built

`ScanBatcher` (`project/watch.rs`):

- `SCAN_WINDOW = 250 ms` — the window, a stated constant (D3). A window opens at the first change in a directory that has none open and closes exactly `SCAN_WINDOW` later. It is measured from the first change, not reset by each one, so a burst longer than the window yields one scan per window instead of starving until the burst ends.
- `GIT_SUBPROCESSES_PER_SCAN = 2` — the second count RFC-026 asks every batching claim to carry. One scan request costs the gate's configuration query and one `check-ignore` (measurement 8, review 431). The batcher runs no git itself; the count is the cost of each request it issues.
- `record(directory, at)` — takes the directory a change happened in, as the caller names it (review 450: the batcher derives nothing from a path). Changes in an open window collapse into it; different directories never share one.
- `drain_due(now)` — returns each closed window as one `ScanRequest { directory, coalesced_events }`, in path order.

### The numbers

Both counts, for each claim, as the test prints them (`cargo test -p tekstide-core --lib project::watch -- --nocapture`):

| Stream | Scan requests | Git subprocesses (steady state) |
| --- | --- | --- |
| 1,000 events into one directory, inside one window | **1** | **2** |
| 1,000 events into one directory, spread over five windows | **5** | **10** |
| Ablated — batching removed (below), the same 1,000-event burst | **1,000** | **2,000** |
| Ablated — batching removed, the five-window stream | **1,000** | **2,000** |

**The git figures are steady state, and the first scan in a process costs one more.** The
verified git executable is cached per process, so the first scan also runs `git --version` once.
Measured against a real scan at review 450 (see below): **3** on the first scan, **2** on every
scan after it. The batching tests count steady state; the one-time extra is not in them.

The burst is a real one, not a loop with nothing between events: its thousand changes are spread through the first half of the window, one every 125 µs, and the batcher is driven the way a real loop drives it (drain, then record, then drain at the end).

### Ablation

`rfcs/handoffs/ablate.sh` on a clean tree, replacing the batcher's `record` body with an unconditional, immediately-due insert:

```
self.pending.insert(directory, PendingScan { due: at, coalesced_events: 1 });
```

Every change now becomes its own scan. Four tests fail, including both counting tests — the burst issues **1,000** requests, the five-window stream **1,000** — which is the number the acceptance asked for. The boundary test and the different-directories test also fail, as they should. The file was restored by the script and the tree was clean afterwards.

### What the tests hold

- `a_burst_of_a_thousand_events_into_one_directory_yields_one_scan_request_per_window` — one request, all 1,000 changes answered by it, two git subprocesses.
- `sustained_churn_costs_one_scan_per_window_it_spans` — five requests across five windows, all changes answered, ten git subprocesses. Six windows of room, so the last window's own scan has time to close.
- `events_for_different_directories_do_not_silently_merge` — two directories, two requests, each answering its own 500 changes.
- `a_window_closes_exactly_one_window_after_the_change_that_opened_it` — not due a nanosecond early; due exactly at the close; a change arriving at the close opens the next window rather than joining the closing one.

### Not in this slice, and why it matters that it is not

No clock is read by any test: every instant is constructed, so the numbers do not depend on machine load. The batcher is not yet fed by anything. D4's evaluation of `notify` (slice B) is judged against these numbers, which is the point of building this first.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `713 + 16 + 1049` = 1,778 passed, 0 failed, 0 entries left after each.** No intermittent.

## Review 450 — the two required items

### 1. The batcher takes the directory it is given

`ScanBatcher::record` used to derive its key as `path.parent().unwrap_or(Path::new(""))`, so a path with no parent became the empty path and went on to a scanner. It now takes the directory the caller names, and derives nothing:

```rust
pub fn record(&mut self, directory: PathBuf, at: Instant)
```

The residual is stated, not hidden: a `PathBuf` can still hold `""`. The batcher does not invent that value — a caller has to hand it — and the only caller (slice B) computes the directory from a real event path under the project root. A stricter newtype would need that root context, so it belongs with B, not here.

The four tests were reshaped to name directories directly, and pass.

### 2. `GIT_SUBPROCESSES_PER_SCAN` against a real scan

Measured, not assumed. `measured_git_subprocesses_per_real_scan` (`#[ignore]`d; run with `cargo test -p tekstide-core --lib measured_git_subprocesses_per_real_scan -- --ignored --nocapture`) runs the explorer's own scan request, through its public oracle seam, with a logging `git` shim as the executable, and counts every subprocess the ignore step spawns:

| Scan | Git subprocesses |
| --- | --- |
| First scan in this process | **3**: `git --version`, `git config --list --null`, `git check-ignore -z --stdin` |
| Second scan, warm | **2**: `git config --list --null`, `git check-ignore -z --stdin` |
| Third scan, warm | **2**: the same two |

**What this corrects.** `GIT_SUBPROCESSES_PER_SCAN = 2` is right for steady state, which is what every batching count in this file assumes. It is wrong for the first scan in a process, which costs 3. The constant's doc comment now states both. The burst and five-window figures above are steady state; with a cold process add one.

**Why the shim is not on the product's own `PATH`.** My first attempt put the shim first on `PATH` and recorded nothing. That is the product working correctly: `resolve_git_executable` tries its reviewed system directories before it reads `PATH`, so a `git` earlier on `PATH` is never reached on this machine. The measurement therefore drives the same ignore step with the shim as the explicit executable, through the seam `ExplorerScanRequest::run_with_oracle` already exposes. The directory read and the ignore step are otherwise the production code.

### Ablation, re-run on the new signature

The batching ablation (every change its own immediately-due scan) on a clean tree, after the signature change:

| Stream | Scan requests |
| --- | --- |
| 1,000 events, one window | **1,000** (was 1) |
| 1,000 events, five windows | **1,000** (was 5) |

Both restored by `ablate.sh`; the tree was clean afterwards.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `rfc_docs_invariants` 16: clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `713 + 16 + 1049` = 1,778 passed, 1 ignored (the measurement), 0 failed, 0 entries left after each**, at machine load 22–25. The first attempt had two runs fail one load-sensitive timing test, `change_review_content_view_build_cost_by_line_count_measurement`, at the same load; registered in `test-process-leak.md` (2026-10-06) and the gate redone rather than counted.

## Slice B — D4's evaluation, before any adoption

**No dependency added. `Cargo.toml` and `Cargo.lock` unchanged.** The evaluation is in
`d4-notify-evaluation.md` in this folder; this section is its evidence summary.

### What was read, and how

`notify` 8.2.0 was fetched with `cargo fetch` into a scratch project under `/dev/shm`, and its source
was read from `~/.cargo/registry`. Nothing was compiled into tekstide and nothing was run. The
dependency tree was taken with `cargo tree -e normal,build` on Linux, default features.

### The three facts

- **MSRV 1.77** (`rust-version`) — verified, under our 1.90 floor.
- **Licence CC0-1.0**, with `LICENSE-CC0` in the crate — verified.
- **Last stable release 2025-08-03** — **not verified here.** The crates.io API returned HTTP 403 and the
  crate has no changelog. The date is carried from RFC-026 D12 and is left unverified in the record.

### What it adds

Five new crates in our lock on Linux: `notify` (CC0-1.0), `notify-types`, `inotify`, `inotify-sys`
(ISC), `mio` (MIT). Five reused: `libc`, `log`, `walkdir`, `same-file`, `bitflags`. The optional
`crossbeam-channel` and `flume` are off by default and absent from the tree.

### What it does when the kernel refuses another watch

Read from `src/inotify.rs`, not from the documentation:

- A non-recursive `watch()` returns `ErrorKind::MaxFilesWatch` (from `ENOSPC`, path attached) **to
  the caller, synchronously**, over a reply channel. The watch is not recorded, so no partial state.
- A recursive watch can fail halfway with the earlier directories already watched. Not used.
- The per-user inotify instance limit (1,024 here) fails at construction.

### Two hazards the design must answer

- **H1 — no `catch_unwind` on notify's event thread.** A handler panic, or a `poll(2)` failure
  (`panic!("poll failed")`), kills the loop, and every later `watch`/`unwatch` panics in our thread
  at `unwrap()`. The handler must be panic-free, and the wrapper must catch a panic from a watch call
  and turn it into D2's sentence.
- **H2 — the real refusal cannot be forced on this machine.** `max_user_watches` is 524,288 and only
  root can change it. The exhaustion test therefore runs against a fake `WatchBackend` that refuses on
  demand. The real `ENOSPC` path is **evidenced by reading the source, not by a test**.

### Checklist

- [ ] **D4 recorded before adoption** — the record exists; **the box stays unticked** because the last
  stable date is unverified. A box naming a fact with no evidence is not ticked. The reviewer can
  verify the date from crates.io and tick it.
- The `dependency-advisories.md` row, the watch scope, the budget-exhaustion test, the hostile
  fixture, and the subscription are all unbuilt: they follow the architect's decision, not this one.

## Slice B1 — `notify` adopted; the wrapper and the scope, tested

**Adopted at review 452.** `notify` 8.2.0, default features off, in `tekstide-core`'s dependencies.

### The dependency, as the lock records it

- **Linux build: five new crates**, as the evaluation said: `notify`, `notify-types`, `inotify`,
  `inotify-sys`, `mio` (confirmed with `cargo tree -p tekstide-core -e normal,build --target
  x86_64-unknown-linux-gnu`).
- **Lock: 18 new entries**, because `Cargo.lock` records every platform. The other 13 are `kqueue`,
  `kqueue-sys`, `windows-sys`, `windows-targets`, eight `windows_*`, and `wasi`. None is compiled on
  Linux. The advisories register states both counts.
- **`cargo audit`:** zero vulnerabilities; the warnings are the three the register already carries.

### The seam (`project/watch/backend.rs`)

- `WatchBackend` — two methods, non-recursive, per directory. Every platform call goes through it.
- `WatchRefusal` — **one shape for every cause** (review 452, C1). `detail` is for logs and tests; the
  type offers no way to branch on the cause.
- `NotifyBackend` — notify's recommended watcher. Its event handler does nothing but send into a
  channel, and drops a send error: there is no panic path on notify's thread (H1).
- `guard_watch_call` — runs each watch call under `catch_unwind` and turns a panic into a refusal (H1).
  Tested with a real panic.

### The scope (`project/watch/scope.rs`)

- `WatchScope::reconcile(desired, backend)` makes the watched set **equal** the desired set: removals
  first, so a shrinking scope frees budget before a growing one needs it. The first refusal stops
  watching: every watch is dropped, the state becomes `Stopped`, and the call ends.
- `resume()` — the only way out of `Stopped`. A reconcile does not retry the platform on its own.
- `desired_directories(root, expanded, open_documents)` — the root, the expanded folders and the
  open documents' folders (D1). Relative paths are joined onto the root.

### Tests (seven new, plus the real-kernel one)

| Test | Proves |
| --- | --- |
| `expanding_collapsing_and_closing_move_the_watched_count_exactly` | the count moves by exactly what each step names; closing removes all of them (R6) |
| `an_open_documents_folder_is_watched_without_being_expanded` | D1's third input |
| `a_refused_watch_stops_watching_and_drops_every_watch_without_crashing` | a forced budget exhaustion (R3): stopped, empty, nothing left on the platform, no panic |
| `the_degradation_does_not_depend_on_which_refusal_it_gets` | C1: two different refusals, identical state |
| `a_stopped_scope_does_not_retry_until_it_is_resumed` | no silent retry; a reopen places the watches again |
| `a_panic_inside_a_watch_call_is_a_refusal_not_a_crash` | H1, with a real panic |
| `the_real_backend_places_and_removes_a_watch_on_a_real_directory` | the production backend against the real kernel, on a directory this process owns |

The real-kernel refusal (`ENOSPC`) is **not** a test: the limit cannot be lowered here (H2, and review
452 accepted that). The degradation is tested through the fake, whose refusal the policy handles exactly
as it handles the real one.

### Ablation

`ablate.sh` on a clean tree: the first refusal does not stop watching (`Err(_refusal) => {}`). Three
tests fail — `a_refused_watch_stops…`, `the_degradation_does_not_depend…`, and
`a_stopped_scope_does_not_retry…`. Restored by the script; the tree was clean afterwards.

### Not in this slice

The scope is **not yet wired** to the live explorer tree or the open-document set; the sidebar
sentence is not written; the event subscription (D10) and the batcher's first real feed are not built;
the hostile fixture is not run. Each is a later B step, and each keeps its checklist box open until it is
done.

### Gate

`cargo fmt --all --check`, `clippy --workspace --all-targets -D warnings`, `rfc_docs_invariants` 16:
clean. **Three consecutive full-workspace runs, `--no-fail-fast`, fresh short `TMPDIR`: `713 + 16 + 1056`
= 1,785 passed, 1 ignored (the measurement), 0 failed, 0 entries left after each**, at load 6–9.
