# RFC-058 QA evidence

## PR-058-A — reproduce, and nothing else

**Watched happen between two real OS processes, not reasoned about from `app.rs` and
`recovery/record.rs`** (D2, the RFC-066 D5 lesson applied before the fact). A new test-only
`[[bin]]`, `project_process_probe` (`crates/tekstide-core/src/bin/`, the same shape as the
existing `reference_adapter`), runs the exact production sequence a real boot already runs. Two
real processes, held genuinely *overlapping* (not sequential — see the required fix below),
sharing one real state root and one real project root, each dirty the same `doc.txt` with
different text.

**It reproduced on the first attempt.** Both processes computed the same project id; the records
directory held exactly one file; its content was the second process's text; the first's own edit
was gone, with nothing on either process's output or in the record itself saying so.

**Review 513's own required fix**: the first version waited for the first probe to exit
(`Command::…output()`) before starting the second, which reproduced two *sequential* sessions, not
two instances holding one project at once — not the real hazard, and not something a correct
PR-058-B fix would still fail against. Fixed with `--hold`: the probe prints its own outcome line,
then blocks on stdin until closed. `HeldProbe::assert_still_holding()` runs on both sides of the
second probe's own run, each a real `libc::kill(pid, 0)` liveness check
(`test_support::process_is_alive`), proving overlap rather than assuming it.

**A second, unrelated gate failure found while fixing that**: the first version of `--hold`
blocked with `std::io::stdin().read_to_end(...)`, which tripped
`project::diff::tests::enumeration_confirms_only_the_closed_list_reads_full_file_content` — a scan
matching the literal text `read_to_end(` against every non-test file, unable to tell a stdin drain
from a file read. The fix was not to add this file to `FILES_ALLOWED_TO_READ_FULL_FILE_CONTENT`
(which would misstate what the code does); a `read_line` loop to `Ok(0)` blocks on EOF the same
way without writing the matched pattern at all.

No product code touched in this slice.

## PR-058-B — the mechanism

### D3/D4 — the lock itself

New module `tekstide_core::project_lock`: a real OS `flock` (`std::fs::File::try_lock`, the same
primitive RFC-050's transcript writer already uses) under
`<state_root>/project-locks/<project_id>.lock` — process-visible (proven by a second,
independent `File::open` + `try_lock` seeing it held, the identical technique the transcript
lock's own test uses) and project-scoped (different ids, or the same id under a different state
root, never collide).

**D4 proven against a real killed process**, not only an orderly drop:
`a_real_sigkilled_holder_releases_the_lock` spawns a real child holding the lock, confirms it held
from an independent handle, **`SIGKILL`**s and reaps it, confirms the pid genuinely left the
process table, and confirms the lock is free again — within the same bound (5s/10ms poll)
`transcript::tests::a_live_writer_holds_an_exclusive_lock_until_it_is_dropped` already established
for the identical fork-pressure flake, read first as the task breakdown required. That same bound
had to be applied to `dropping_the_lock_releases_it_for_a_later_acquire` too, found by the test
itself flaking once in dev under exactly the sibling-test fork pressure that precedent documents.

### D5 — the one deliberate difference from that precedent

`BoundedTranscriptWriter::create` treats *any* lock failure as "do not write." This module treats
only `WouldBlock` as held; every other outcome (unreadable directory, unsupported locking, any
other I/O error) is `CannotDecide`, and the caller opens the project unprotected rather than
refuse. The lock file's own content (the holder's pid, for naming later) plays no part in the
exclusivity decision — only `try_lock` does — so stale or unparseable content can never block
opening, by construction rather than by special-casing. Each of the three named ambiguous cases
has its own test: a lock naming a pid nobody actually holds, content this build cannot parse, and
an unreadable locks directory.

**Writing the third one caught a real bug first.** The first version of `acquire_project_lock`
unconditionally reset the locks directory's permissions to `0o700` after creation (copied from
`recovery::write_recovery_record`, where that reset is a deliberate privacy measure) — which
silently repaired the test's simulated-unreadable directory *before* the open attempt, so the
"must report `CannotDecide`" test failed by getting `Acquired` instead. Fixed by setting the mode
only via `DirBuilder::mode()` at the moment of creation (which only ever governs a directory the
call itself creates), never resetting an already-existing one afterward.

### D1 — the audit store, confirmed rather than duplicated

`audit::tests::two_live_writers::two_concurrent_writers_against_the_same_store_both_succeed`: two
independent `AuditStore` handles against the same real sqlite file, barrier-synchronized onto the
same `append` instant, both succeed, both records read back. `audit/store.rs` itself untouched.

**A real, narrower hazard found and correctly diagnosed on the second attempt.** The first version
raced the two handles' own `AuditStore::open` calls too (no synchronization before `open`, only
before `append`) and hit a genuine `AuditStoreError { reason: Io }`. First attributed (wrongly, at
review 515) to a `busy_timeout` gap; the real cause is a TOCTOU in `AuditStore::open_internal` —
both connections read `database_file().exists()` before either opens a connection, both see
`false`, both reach `create_current_schema`, whose `CREATE_SCHEMA_V3` has no `IF NOT EXISTS`
anywhere, so the second `CREATE TABLE` is correctly rejected. Corrected in the RFC's own "What is
actually at risk" table (narrowed from "Yes, already" to "Yes, for an established store") and
recorded in `rfcs/future-work.md`, beside review 503's own audit finding — out of this RFC's own
scope (the audit store is app-wide, not project-scoped; `acquire_project_lock` could never have
guarded it regardless of the exact cause), but resting on the real cause this time.

### The regression test

PR-058-A's own reproduction, rewritten rather than duplicated:
`two_real_processes_opening_the_same_root_share_an_id_but_the_second_is_blocked_from_writing`. Same
two overlapping real processes; the second now finds the project locked and never writes at all —
the record that survives is the first process's own text, unclobbered.

## PR-058-C — saying it

### D8 — the second attempt names the holder and opens no duplicate

`AppState::add_project_from_path_protected` wraps `add_project_from_path`: on `Added`, it attempts
`acquire_project_lock` against the exact id that call just reused or minted — never a second,
independently-computed id that could disagree with it; on `HeldByAnother`, it undoes the add
(`remove_active_project_session`) before returning `ProjectOpenOutcome::Blocked`, so no duplicate
session is ever left in `self.projects`. All three real GUI open call sites (`reopen_recent_project`,
`attempt_open_project_from_path_field`, `choose_current_browsed_directory`) and the CLI path
(`open_cli_project_path_and_record`) now call this instead of the unprotected version, falling back
to it only when the state root itself cannot be resolved (D5, the same degradation every other
real-state-root consumer in this crate already has).

Proven at the GUI level, not only at the `tekstide-core` API level:
`a_second_attempt_at_a_locked_project_opens_no_duplicate_and_says_nothing_false` holds a real lock
(standing in for a second live process, the same two-handle substitution
`project_lock::tests`/the audit-store confirmation above both already rely on), drives the real
`Message::ReopenRecentProjectRowPressed`, and asserts `state.app_shell.state().projects()` is
empty and nothing became active.

### D9 — the attention request, worded so it never claims what did not happen

Checked in the dependency tree actually in use (not assumed): `iced 0.14`'s `window::gain_focus`
reaches winit's `focus_window`, documented **"Wayland: Unsupported."** Only
`window::request_user_attention` works there. `Message::ProjectAttentionRequested`'s own handler
calls it best-effort against `state.window_id` (learned once from `iced::window::open_events()`);
`None` is a silent no-op, never an error.

**The rendered string is checked, not the intent behind it** (the checklist's own instruction).
`project-board-open-blocked` = *"This project is already open in another Tekstide window. It has
been asked for your attention."* — a sentence that is true whether or not the compositor shows
anything, because it describes this process's own action (the knock it sent), not a promise about
what the other window does. The same GUI test above asserts the rendered text contains none of
"raised", "switched", "focused", "activated" (case-insensitive), and does say "already open" and
"attention."

### D10 — the IPC reaching the holder

New `tekstide_core::project_lock::attention` module: a real `AF_UNIX` listener bound alongside the
lock, at `<state_root>/project-locks/<project_id>.attention.sock`. **Deliberately not
`approval::channel`'s own `/proc/self/fd`-relative hardened bind** — that module guards a
capability token an untrusted adapter process holds; this channel carries no token and decides
nothing, so a plain path-based bind is proportionate, with the one property reused from that
precedent: the socket-path-length check (`max_socket_path_len`, a second, independent three-line
copy rather than a shared dependency between two otherwise-unrelated modules).

Proven end to end between two real processes, not three separate claims taken on trust:
`a_blocked_second_process_names_the_real_holder_and_its_knock_is_received` — the second process's
own `BLOCKED <pid> KNOCKED` status line names the *real* pid of the held probe (parsed back out of
the rendered line, not asserted only internally), and the held probe, released afterward, reports
`ATTENTION_RECEIVED` on its own attention listener — the other half of the same round trip, not
merely that the knocker's `connect()` returned `Ok`.

### `REQ-PROJ-009`

Returns to the Project-lifecycle coverage row in `rfcs/delivery-plan.md`, naming what the
mechanism actually is (a real `flock` plus a real `AF_UNIX` knock), not "a lock exists now."

## Gate

`cargo fmt --check`, `cargo clippy --workspace --all-targets -D warnings`: clean.
`rfc_docs_invariants`: 19/19. `i18n::enforcement` (23/23, one new key, confirmed used and
translated). `cargo test --doc --workspace`: clean. Three consecutive full-workspace runs, fresh
short `TMPDIR` each: `759 + 19 + 1132`, 0 failed, 0 fixture entries left, clean on the first
attempt (after the `trigger_git_summary_refresh` enumeration test caught a literal duplicate call
site in `main.rs`'s own fallback branch, factored into a shared helper the same way the three real
GUI call sites already were).
