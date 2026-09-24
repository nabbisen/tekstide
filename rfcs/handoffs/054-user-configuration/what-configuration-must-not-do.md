---
title: "What configuration must not do"
rfc: "RFC-054"
rfc_file: "../../accepted/054-user-configuration-completion.md"
source_rfc_status: "Accepted 2026-09-24 — M12 (closes it)"
target_milestone: "M12"
created: "2026-09-24"
---

# What configuration must not do

**Required reading before writing code.**

## §1 It must not come from the repository

The configuration path is the user's own config directory and nothing else. **A project must never be
able to rebind a key, restyle the window, or name a font.** That holds today because no such path
exists; the moment a file can do those things it is worth attacking, so **pin it with a test** and an
ablation that fails if the loader ever looks beside the project root.

## §2 It must not make the product unreadable

A colour pair below **4.5:1** falls back to the default and says so, with the **measured ratio** in
the diagnostic. A font size outside **8–32 px** falls back. `NFR-UX-003` is a requirement, not a
preference, and a user who cannot read the window cannot fix the file that broke it.

## §3 It must not open a file we then parse

**A font is a family name, resolved by the system font database — never a path.** A font file named
in configuration is a parser eating user-named bytes at startup, and this project does not do that
without a reason (RFC-016, RFC-024, RFC-030 all turn on the same point).

## §4 It must not lose an action

A rebind that collides with another rule, or names a `Reserved` chord, **refuses with a diagnostic
and the default stands**. Last-wins would make an action silently unreachable, which is the exact
failure `future-work.md` records twice.

## §5 It must not leave a status that means two things

`Configurable` with a `None` binding has meant **dead** — and read as *bindable* — since RFC-022.
When this slice is done, an action has a real default binding **or** an explicitly dead status, and
the two are distinguishable **by the type**, not by noticing a `None`.

## §6 It must not echo a configured string raw

A family name, a profile name, a chord spelling — all of it is input. Diagnostics go through
`text_safety::quote_untrusted` like every other untrusted string this product renders.

## §7 It must not change structure

No new actions, no new surfaces, no colour carrying meaning alone. A theme may restyle a badge; it
may not remove the word inside it. The i18n completeness and colour-alone scans still pass.
