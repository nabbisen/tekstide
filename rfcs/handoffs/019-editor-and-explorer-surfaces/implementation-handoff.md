---
title: "RFC-019: Editor and Explorer Surfaces - Implementation Handoff"
rfc: "RFC-019"
rfc_file: "../../done/019-editor-and-explorer-surfaces.md"
status: "Accepted 2026-08-10 — ready for implementation"
target_milestone: "M10"
created: "2026-08-10"
---

# What exists, what is missing, and where the seams are

## Already built, in `tekstide-core`

| Item | Where | State |
| --- | --- | --- |
| `TextDocument` — `text()`, `state()`, `is_dirty()`, `cursor()`, `viewport()`, `set_cursor()`, `last_known_snapshot()` | `content/document.rs` | complete |
| `TextCursor`, `TextViewport`, `TextDocumentState`, `ExternalChangeDecision` | same | complete |
| `TextDocumentOpenPolicy`, `DEFAULT_MAX_EDITABLE_BYTES` (4 MiB), `TextDocumentOpenError` | `content/open.rs` | complete |
| `TextDocumentEditError` | `content/edit.rs` | complete |
| Save and snapshot models | `content/save.rs`, `content/snapshot.rs` | complete |
| `ProjectContentWorkspace` — explorer scan, active document, status, dirty/open counts | `project/content.rs` | complete |
| `ProjectExplorerStatus`, `ProjectContentStatus`, `ProjectContentError` | same | complete |
| `AppState::open_active_project_text_document` / `scan_active_project_explorer_directory` / `replace_active_project_text` / `save_active_project_text_document` / `refresh_active_project_text_document` | `app.rs` | **complete, no production caller** |

## Genuinely missing

1. **Any rendered content surface.** Content mode has been a catalog-driven placeholder
   since `0.4.0` (`main_area_key` → `main-area-content-mode-placeholder`). There is no
   editor widget and no tree widget in `crates/tekstide`.
2. **Catalog keys for content and explorer state.** None of the six core label producers
   has a `.ftl` counterpart yet.
3. **Input routing for an editing surface.** RFC-015's `SurfaceInput`/`TextStream` classes
   exist; `TextStream` currently serves the terminal only.

## Seams to reuse rather than rebuild

**The surface contract.** `surface/board.rs` is the closest working example of a surface
that renders core state without duplicating it, escapes untrusted text
(`text_safety::quote_untrusted`, line 135), and routes every word through `Catalog`.
Read it before writing either new surface.

**Input routing.** `route_non_modal_input` is pure and has no `ApplicationShell` access —
that is deliberate and is what makes `input.rs` unable to address trusted state. If an
editing action needs project state, resolve it in `shell.rs` and keep `input.rs` ignorant,
the way PR-018-B did for paste after correctly declining the shape the handoff suggested.

**The catalog pattern.** `session-bar-entry` in `en.ftl`: one Fluent message with select
expressions, the Rust side naming a branch and the `.ftl` file supplying the words. Not
several lookups concatenated in Rust — concatenation hardcodes English word order even
when every word is catalogued.

**Modal exclusivity.** If any part of this RFC opens a dialog (an external-change prompt
is the likely candidate), it goes on the existing modal layer as a `ModalContent` variant
— **not** a second `Option` field on `State`. `ModalAbsent`/`for_modal` are generic over
content type, and only the enum keeps the single gated value RFC-015 built. PR-018-C's
note to RFC-022 explains this at length.

## Things that will bite

**File content is untrusted, and so are file names — differently.** See
[`the-escaping-asymmetry.md`](./the-escaping-asymmetry.md). This is the one thing in this
RFC that is easy to get half-right.

**The explorer renders a directory a user did not create.** Node counts, name lengths, and
nesting depth are all attacker-influenced. `ExplorerDirectoryScan` is core's and already
bounded; render what it gives you rather than walking the filesystem yourself.

**External change is a decision, not an error.** `ExternalChangeDecision` exists because a
file changing underneath an open buffer is a situation with more than one right answer.
Rendering it as a failure, or resolving it silently, both throw away the model.

**Do not touch terminal code.** `NFR-PERF-004`, the three-terminal limit, the 10 ms
`WouldBlock` sleep and the 64 KiB cap are one coupled change owned by readiness-driven
terminal I/O (`../../future-work.md`). Nothing in this RFC should go near them, and the
closeout may not claim any performance change.

**Symlink status is security-relevant.** `FileAccessSymlinkStatus` exists because a
symlink in a project can point outside it. Whatever the explorer shows about it must be
escaped like any other chrome, and RFC-019 §Open questions 2 leaves the "show the target
or not" decision to PR-019-B — to be made with escaping already in place, so the question
is about usefulness rather than risk.
