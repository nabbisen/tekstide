---
title: "What a path field must not trust"
rfc: "RFC-038"
rfc_file: "../../accepted/038-first-run-and-project-entry.md"
source_rfc_status: "Accepted 2026-08-24 — M12, first"
status: "Required reading before any RFC-038 code"
created: "2026-08-24"
---

# What a path field must not trust

This project has shipped a text-rendering surface before. It has not shipped one that **takes
text in and gives it back**. That is a different threat surface and this document exists so it
is treated as one.

## 1. The typed path is untrusted, even though the user typed it

The instinct is that a user's own input cannot attack them. It can, for three reasons:

- **It is echoed back.** Whatever is typed appears in the field, and on failure appears again
  in a diagnostic. A path containing `U+202E RIGHT-TO-LEFT OVERRIDE` renders one way and means
  another — the Trojan Source class RFC-016 exists for. A screenshot of that board, pasted into
  an issue, misleads whoever reads it.
- **It is not always typed.** It is pasted, and clipboard content has an origin the user did
  not choose. RFC-018 built an entire paste-protection model on exactly that premise; nothing
  about a path field makes the clipboard trustworthy again.
- **It becomes a persisted display string.** The project lands on the board and its name and
  path are rendered on every subsequent launch, from the recent-projects cache.

**Required:** the same treatment every other untrusted string in this codebase gets —
`text_safety::quote_untrusted` on render, never handed raw to `text(...)`. There is no new
policy to invent here; there is an existing one to not bypass.

## 2. The failure path must not exit the process

Today `add_project_from_path`'s `FailClosed` symlink refusal reaches `eprintln!` and
`std::process::exit(1)` in `boot()`. That is correct for a CLI argument — no window exists yet,
and aborting beats booting with a half-applied argument list.

**Reached from a text field it is catastrophic: a typo would close the application.**

**Required:** the field's failure path renders a diagnostic and leaves the application running.
`boot()`'s existing CLI behaviour is unchanged — do not "unify" the two paths. They have
genuinely different correct answers, and this is the second time this project has found that a
behaviour correct in one context is a defect in another (`Child::drop`, `test-process-leak.md`).

## 3. The diagnostic is bounded, not just escaped

An error message that embeds an arbitrary-length attacker-influenced string is its own problem:
it can push the rest of the surface off-screen, and it can carry thousands of combining
characters.

**Required:** follow RFC-023's `bound_key_segment` exactly — it exists for this and is already
reviewed:

```rust
let truncated: String = raw.chars().take(MAX).collect();
let was_truncated = raw.chars().count() > MAX;
let mut bounded = escape_untrusted_chars(&truncated);
if was_truncated { bounded.push('\u{2026}'); }
```

Truncate **then** escape, and mark truncation visibly. Do not write a second escaping routine;
response 269 already rejected that once in RFC-023 and the reasoning has not changed.

## 4. Adding a project must not grant it anything

A project added through this field arrives `Restricted`, exactly as one added from the CLI does.
The field is an *entry* mechanism, not a trust decision, and RFC-032 owns granting.

**Required:** a test proving a project added through the field is `Restricted` and that an agent
run in it is refused until trust is granted through the existing `Ctrl+Alt+U` route. If that
test is awkward to write, that is a signal worth escalating, not routing around.

## 5. Every call to `add_project_from_path` writes an audit record

`project_added` is wired at the *call site*, not inside the operation — `AppState` holds no
`AuditCoordinator`. `add_project_from_path_is_called_exactly_once_from_main_rs_and_nowhere_else`
guards this by **occurrence count**, and its doc comment predicted this slice: *"an interactive
'Add Project' flow would compile and work with no record and no error."*

**Required:** wire the record on the new path, and update the guard to name both call sites with
their exact counts. Keep it a count. Relaxing it to a per-file presence check would let a second
unreviewed call inside an already-allowed file pass silently — the precise defect response 264
corrected in RFC-031.

## 6. What this document does not cover

It says nothing about whether the *path resolution* is correct — canonicalisation, symlink
policy, and root validation are `add_project_from_path`'s own, already reviewed under RFC-005
and RFC-032. **Do not reimplement any of it in the surface.** The field's job is to collect a
string and hand it to the existing validated entry point. If you find yourself calling
`canonicalize` in `shell.rs`, stop and escalate: that is the surface taking a security decision
that belongs to core.
