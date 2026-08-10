---
title: "RFC-019 — The escaping asymmetry: read before PR-019-B or PR-019-C"
rfc: "RFC-019"
rfc_file: "../../proposed/019-editor-and-explorer-surfaces.md"
status: "Ready for implementation — accepted 2026-08-10 with the RFC"
created: "2026-08-10"
---

# The escaping asymmetry

This is one document rather than two because **the two halves are only correct
together**, and getting one right while getting the other wrong is the most likely way
this RFC ships a defect.

## The rule, in one line each

**The explorer tree escapes. The editor's text area does not.**

Both are decided already — the second by RFC-016, quoted in RFC-018's §The security core
and again in RFC-019 — so neither is yours to re-derive. Your job is to implement them
without letting either leak into the other's territory.

## Why the editor must not escape

RFC-016 §Text safety by surface:

> **Editor surface — Do not escape.** The user is editing real file content; they must
> see it as it is. Bidi reordering is correct behaviour here.
>
> The editor exception is deliberate: an editor that silently rewrites file content is
> broken.

An editor that escapes is not a safer editor. It is a **broken** one: the user sees
`<U+202E>` where their file has a character, edits around it, saves, and the file now
differs from what they intended. Escaping here corrupts data.

RFC-014 C10 verified this substrate reorders bidi correctly via
`cosmic-text`/`unicode-bidi`. That is the behaviour to render, not a risk to mitigate.

**"Show invisibles" may be built. It may not be called a security control.** RFC-016 says
so explicitly, and RFC-019 repeats it. A marker the user can toggle off is not a
boundary, and describing it as one would be the kind of claim this project's honesty
gates exist to catch.

## Why the explorer must escape

A repository can contain a file named:

```
proj<U+202E>gpj.exe
```

which renders to a reader as `projexe.jpg` — an executable that looks like an image. The
name is **attacker-influenced**: it arrives with a cloned repository, and nobody typed
it.

The explorer tree is **trusted chrome**. Every name, path hint, and status string it
renders goes through `tekstide_core::text_safety::quote_untrusted`. `surface/board.rs:135`
is the live example to copy.

**This is not hypothetical here.** A name of exactly that class already sits in this
project's own recent-projects state and has been exercising the board's escaping on every
launch since it was created. Test the explorer against the same class deliberately.

## The line between them

Everything on screen is chrome **except the editor's text area**.

| Surface region | Escapes? |
| --- | --- |
| Editor text area | **No** — raw, bidi reordered |
| File path shown above/around the editor | Yes |
| Dirty-state indicator, tab label | Yes |
| Explorer node names | Yes |
| Explorer status and symlink state | Yes |

If you find yourself unsure which side something is on, the test is: **is the user
editing these bytes?** If yes, raw. If it is Tekstide describing something, escaped.

## Test both directions, not one

The asymmetry means each half has an opposite failure, and a test suite that only checks
one direction will pass while the product is broken:

- **The editor must fail a test that asserts escaping happened.** Render a document
  containing `U+202E` and assert the text area contains the **raw** character — that a
  future well-meaning change adding `quote_untrusted` here breaks a test rather than a
  user's file.
- **The explorer must fail a test that asserts raw rendering.** Render a node named with
  `U+202E` and assert the escaped form appears and the raw character does not.

One ablation each. If either test passes with its escaping decision inverted, the test is
not testing the property.

## The label trap, which lands in this slice

RFC-019 names six hardcoded-English producers in `tekstide-core`. **Four are free
functions that the existing scan does not catch**, because
`no_count_display_or_attention_label_is_called_anywhere_in_the_crate` matches the literal
substring `.label()`:

- `explorer_node_kind_label(kind)`
- `explorer_node_state_label(state)`
- `explorer_symlink_status_label(status)`
- `text_document_state_label(state)`

They are exactly the shape that shipped ten hardcoded English strings into the session bar
in RFC-017 PR-017-E, caught only in review. **You now know in advance.** Every user-facing
word goes through `Catalog`, using `session-bar-entry`'s one-message-with-selectors
pattern — the Rust side names a branch, the `.ftl` file supplies the words.

Do **not** widen the scan to catch free functions as part of this slice. That is
`i18n::enforcement`'s territory and RFC-019 §Open questions 1 asks you to raise it, not
absorb it. A scan widened under a rendering RFC is a scan nobody owns.
