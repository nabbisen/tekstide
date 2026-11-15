---
title: "Sweep: which recorded findings are already closed, checked against the code"
status: "Audit output, 2026-11-15, ordered by the owner after three queued items in a row turned out to be already done. Findings only; no register was rewritten."
rfc_file: "none — audit output"
target_milestone: "M14"
created: "2026-11-15"
---

# Sweep of the recorded findings

**Why.** Three checks in a row found queued work already satisfied — the `cargo doc` gate (in CI
since the day the entry was filed), `REQ-SEC-023` (satisfied vacuously), and the threat model's
Q10 (answered by the export dialog). The reviewer twice came close to scoping work against an item
that was already closed. This sweep checks the rest **against the code**, before anyone else does.

**Method.** Every standalone finding in `future-work.md` and every entry in `rfcs/README.md`'s
reserved table, each checked by grep or by naming the RFC that shipped it. Nothing here is
inferred from a document saying so.

## The headline, and it corrects the claim that prompted this sweep

**The reserved RFC table is not stale. Every one of its entries is genuinely open.** The staleness
was concentrated in `future-work.md` and in *descriptions* of reserved items, not in the
reservations themselves. The reviewer's "the register is carrying closed items" was broader than
the evidence supported, and this sweep narrows it.

## Reserved RFCs — all nine open, verified

| RFC | Checked by | Open? |
| --- | --- | --- |
| 061 combining-marks bound | `config/terminal.rs` names it as **not covered**, explicitly | **Open** |
| 062 `core.excludesFile` | No production handling; the only match is a docs invariant | **Open** |
| 063 `iced_selector` | No such dependency or capability anywhere | **Open** |
| 064 does `iced`'s `text_editor` replace ours | A question, not work; nothing to satisfy | **Open by nature** |
| 068 redaction | **Re-characterised**, see Part 2's F5 correction: `REQ-SEC-023` is met, the scanner is out of scope, the disclosure already exists | **Not needed as reserved** |
| 070 check for updates | **No outbound network client exists at all** — the only matches are OSC-8 hyperlink *classification* in the terminal parser | **Open**, and it would still be the first network request |
| 071 Windows packaging | No `target_os = "windows"` arm anywhere | **Open** |
| 072 non-blocking job for quarantined tests | Open, but its own premise is doubtful: the 26 `#[ignore]`s are mostly measurements and hand-run diagnostics, as its reservation already warned | **Open, premise questionable** |
| 075 attention knock | `request_attention` still has **no production call site** — only `project_process_probe`, a diagnostic bin | **Open**, exactly as recorded |

## `future-work.md`'s standalone findings — four closed, seven open, one addressed differently

**Closed, and the entry does not say so:**

| Finding | Closed by |
| --- | --- |
| Confirming a close with an unsaved file kills terminals and does not close (1132) | **RFC-066**, shipped `0.32.0` |
| Two processes racing to create the audit store, second loses (1251) | Fixed; `two_real_processes_racing_to_create_the_store_both_succeed` exists and passes |
| Non-`en` locales fail outside `cargo install` (1488) | **RFC-073**, shipped `0.40.0` |
| `cargo doc` not in the gate (1547) | Corrected 2026-11-15: the gate step has existed since `67704ea`, the day the entry was filed |

**Still open, verified rather than assumed:**

- A user cannot remove a recent project (1199) — the only `remove_*` functions touch recovery
  records, not the recent list.
- The audit directory and its files have no explicit permission mode (1369) — no `0o600`/`0o700`
  or `set_permissions` anywhere under `audit/`. Worth noting the contrast: the run-report export
  **does** set `0o600`, so the product knows the idiom and has not applied it here.
- The window sets no Wayland `app_id` (1446) — still none in `crates/tekstide/src`.
- A `proposed/` RFC's status claim is unchecked in every register (1330) — confirmed in the code:
  `every_pack_status_field_agrees_with_its_rfc_folder`'s match covers `"done"` and `"accepted"`
  and falls through to `false` for everything else, so `proposed/` is genuinely unchecked.
- RFC-063's motivating case (1174), and the two remaining items this sweep did not re-derive
  independently (1222's silent-drop half, 1283's claim-site count) — see below.

**Addressed in a different form than the finding proposed:**

- An audit write rejected by SQLite and silently dropped (1222) — `AuditIntegration::record_write_failure`
  now exists, so a failed write is recorded rather than dropped. Whether it reaches the *user* was
  not re-derived here and the entry should be updated by whoever does.
- The audit view's filter lists do not fail to compile when a variant is added (1412) — guarded by
  a named invariant (`all_families_cover_every_declared_cycle_index_…`) over a fixed
  `ALL_FAMILIES: [AuditEventFamily; 12]`, i.e. **by test rather than by compilation**, which is
  weaker than the finding asked for and stronger than nothing.
- Four of seven release-state claim sites unchecked (1283) — two of the named checks now exist
  (`every_readme_row_agrees_with_the_changelogs_release_state`,
  `the_newest_release_status_matches_whether_it_was_actually_published`). Whether all seven sites
  are covered was not counted here.

## What this sweep deliberately did not do

**It did not edit `future-work.md`.** Marking four findings closed changes a record of what was
found and when, and this project's convention is a dated correction by the person who establishes
it — not a sweep silently retiring entries. The four closures above are each traceable to a
shipped RFC or a named passing test; whoever applies them should say so in the entry, dated.

It also did not re-derive 1222 and 1283 to a conclusion, and says so rather than guessing.

## The finding worth carrying forward

**This project's documents record findings faithfully and close them silently.** Four entries name
defects that shipped fixes months ago, and the fix never came back to the entry. That is the same
drift `future-work.md` exists to catch, pointed at itself — and it is why a reviewer scoping from
it reached twice for work that was already done.

The cheap corrective is not a process: it is that **the release that closes a finding says so in
the finding**, the same way RFC status lines already move with their folder.
