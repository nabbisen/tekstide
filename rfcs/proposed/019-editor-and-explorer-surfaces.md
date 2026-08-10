# RFC-019: Editor and Explorer Surfaces

Status: Proposed — awaiting the human owner's acceptance
Target milestone: M10 (`0.6.x`)
Date: 2026-08-10

Related baseline documents:

- `tekstide-requirements-v0.md`
- `tekstide-external-design-v0.md`
- `tekstide-uiux-wireframes-v0.md`
- `tekstide-security-threat-model-v0.md`
- [`ROADMAP.md`](../../ROADMAP.md) §M10

Depends on:

- [RFC-006](../done/006-projectsession-state-and-file-explorer-editor-basics.md) — the document, cursor, viewport, and explorer models. **This RFC renders them; it does not amend them.**
- [RFC-015](../done/015-application-shell-and-rendered-surface-model.md) — the surface contract and input-routing model.
- [RFC-016](../done/016-internationalization-and-localization.md) — text safety, and **the editor's already-decided exception to it**.

## Summary

Render Content mode: a text editor over RFC-006's `TextDocument`, and a file-explorer
tree over its `ExplorerDirectoryScan`.

**This is the third RFC in a row with the same shape**, and saying so up front is the
most useful thing this document can do. RFC-006 already defines the model.
`tekstide-core` already implements it — `TextDocument` with `TextCursor`, `TextViewport`,
`TextDocumentState`, dirty tracking, `ExternalChangeDecision`, `SaveDecision`,
`TextDocumentOpenPolicy` with a 4 MiB editable bound, and the explorer scan. **None of it
has a production caller.** `open_active_project_text_document`,
`scan_active_project_explorer_directory`, `replace_active_project_text` and
`save_active_project_text_document` are all unused outside tests — exactly the condition
`plain_terminal_observation` was in before RFC-017 PR-017-F, and `TerminalInputPolicy`
before RFC-018 PR-018-B.

If you find yourself writing a policy rule, a bound, or a state machine, stop. It exists
in core, or it belongs there.

## What this closes, and what it does not

**Closes:** RFC-006's deferral of rendered content surfaces; Content mode being a
placeholder since `0.4.0`.

**Does not close:** diff review and the AgentRun report — those are RFC-020, M10's second
half. Nor anything terminal-related: `NFR-PERF-004`, the three-terminal limit, and the
output ceiling are owned by readiness-driven terminal I/O and are untouched here.

## Non-goals

- **Syntax highlighting.** The delivery plan lists it as optional. It is a rendering
  feature with a large dependency surface and no security content; if it lands at all it
  is its own slice, after the editor works.
- **Language servers, multi-cursor, search-and-replace, undo history beyond what RFC-006
  models.** Each is a product in its own right.
- **Editing binary or oversized files.** `TextDocumentOpenPolicy` already refuses above
  `DEFAULT_MAX_EDITABLE_BYTES` (4 MiB). Render the refusal; do not add a second bound.
- **A file-tree write surface** — no rename, delete, or create. Reading a project's
  structure is this RFC; mutating it is not.

## The security core

### The editor's escaping exception is already decided — do not re-litigate it

RFC-016 §Text safety by surface settles this, and it is binding:

> **Editor surface — Do not escape.** The user is editing real file content; they must
> see it as it is. Bidi reordering is correct behaviour here.
>
> The editor exception is deliberate: an editor that silently rewrites file content is
> broken. A future editor-side "show invisibles" affordance is ordinary functionality,
> **not a security control.**

Two things follow, and the second is where implementations usually go wrong.

**The editor renders raw, and reorders bidi correctly.** RFC-014 C10 verified the
substrate does this via `cosmic-text`/`unicode-bidi`. That is the behaviour, not a
concession.

**"Show invisibles" may be built, but may not be described as a defence.** A source file
containing `U+202E` can read differently from how it compiles — the Trojan Source class.
An affordance that reveals it is genuinely useful. **It is not a security control**, and
this RFC may not claim it as one. RFC-016 chose that framing deliberately; a marker the
user can toggle off is not a boundary.

### The explorer tree is chrome, and file names are attacker-influenced

This is the part with real security content, and it is the direct analogue of RFC-017's
grid-not-chrome boundary.

A repository can contain a file named `proj<U+202E>gpj.exe`, which renders as
`projexe.jpg`. **The explorer tree is trusted chrome**, so every name, path hint, and
status string it renders goes through `text_safety::quote_untrusted`.

This is not hypothetical here: a bidi-override name already exists in this project's own
recent-projects state and has been quietly exercising `surface/board.rs`'s escaping on
every launch. **Test the explorer against the same class specifically.**

The line is: **the editor's text area is the exception; everything else on the screen is
chrome.** A path shown above the editor, a dirty-state indicator, a tab label — all
escape.

### The label trap, predicted in advance rather than discovered in review

`tekstide-core` exposes **six** hardcoded-English producers this RFC will be tempted by:

| Producer | Caught by the existing `.label()` scan? |
| --- | --- |
| `ProjectExplorerStatus::label()` | ✅ yes |
| `ProjectContentStatus::label()` | ✅ yes |
| `explorer_node_kind_label(kind)` | ❌ **no** |
| `explorer_node_state_label(state)` | ❌ **no** |
| `explorer_symlink_status_label(status)` | ❌ **no** |
| `text_document_state_label(state)` | ❌ **no** |

`no_count_display_or_attention_label_is_called_anywhere_in_the_crate` matches the literal
substring `.label()`. The four free functions are not method calls, so **the scan will not
catch them**.

That is precisely how `slot_label`/`status_label` shipped ten hardcoded English strings
into the session bar in RFC-017 PR-017-E, caught only in review. This RFC will meet four
more of the same shape, and now knows it.

**Every user-facing word goes through `Catalog`**, using `session-bar-entry`'s pattern:
one Fluent message with a select expression, the Rust side naming a branch, the `.ftl`
file supplying the words. Not several lookups concatenated in Rust — concatenation
hardcodes English word order even when every word is catalogued.

## The surface contract

Both surfaces are RFC-015 surfaces and inherit it without exception. The one that will
bite:

**Cursor and viewport belong to core.** `TextCursor` and `TextViewport` are `TextDocument`
state with `set_cursor` already provided. A shell-local cursor is duplicated state and
breaks the contract PR-017-C held for the terminal pane. The rendering is the shell's;
the position is not.

If core's edit surface turns out insufficient for real editing, **stop and raise it** as
an RFC-006 question. Do not work around it in the shell — that is the shape that produced
two consolidations already.

## Slices

**PR-019-A** — design and handoff acceptance.

**PR-019-B** — the explorer tree. Renders `ExplorerDirectoryScan`; selection drives
`scan_content_explorer_directory`. Gate: every rendered name and status escaped and
catalogued, with a bidi-override case tested specifically; no `*_label` free function
called; symlink and access states rendered without colour alone (`NFR-UX-002`).

**PR-019-C** — the editor, read-only. Opens through `open_text_document`, renders the
document with cursor and viewport from core, and renders the open-policy refusal for
oversized files. Gate: raw text, bidi reordered, **no escaping in the text area**; chrome
around it escaped; cursor state read from core, not duplicated.

**PR-019-D** — editing and save. `replace_active_text`, `save_active_document`, dirty
state, and `ExternalChangeDecision` when the file changed underneath. Gate: the
external-change decision is **rendered and answerable**, not silently resolved — a save
that overwrites someone else's change without asking is the defect this slice exists to
avoid.

**PR-019-E** — closeout. Claim statement checked against this RFC's own text.

Sequencing: **B and C are independent; D needs C.** E needs all.

## Risks

- **The editor escapes when it should not**, breaking the file. Mitigated by RFC-016's
  decision being quoted rather than re-derived, and by PR-019-C's gate testing raw
  rendering explicitly.
- **The explorer does not escape when it should.** Mitigated by a bidi-override test case
  in PR-019-B, against the same class already live in this project's state.
- **Four uncaught `*_label` free functions ship hardcoded English.** Mitigated by naming
  all six in this document before implementation starts.
- **Shell-local cursor state.** Mitigated by PR-019-C's gate.
- **A save silently overwrites an external change.** Mitigated by PR-019-D's gate.

## Open questions

1. **Should the `.label()` scan be broadened to catch free functions?** It would have
   caught `slot_label` and would catch these four. But it is `i18n::enforcement`'s
   territory, not this RFC's, and a scan widened under a rendering RFC is a scan nobody
   owns. **Raise it; do not absorb it.**
2. **Does the explorer render symlink targets?** `FileAccessSymlinkStatus` exists, and a
   symlink's *target* is attacker-influenced text pointing outside the project. Showing it
   is useful; showing it unescaped in chrome is not. Decide in PR-019-B with escaping
   already in place, so the question is about usefulness rather than risk.
3. **Syntax highlighting: at all, and if so when?** Answer at closeout with the editor
   working, not before.
