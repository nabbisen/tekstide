---
title: "What a legible preview must not become"
rfc: "RFC-042"
rfc_file: "../../accepted/042-change-content-legibility.md"
source_rfc_status: "Accepted 2026-08-26 — M12, first of three for 0.15.0"
target_milestone: "M12"
created: "2026-08-26"
---

# What a legible preview must not become

**Required reading before writing code.** RFC-041's own security document says file content is
untrusted text in trusted chrome. This slice is about to give that untrusted text **layout
control**, which is the single largest concession this surface has made to it. Everything here
is about paying for that concession.

## §1 The threat, stated plainly

A user runs an AI CLI agent on a project. The agent writes files. The user opens Change Review to
decide whether to trust what it did.

**The agent chooses the content of what that screen displays.** Not the chrome — the content. If
content can be made to look like chrome, the agent chooses what the screen appears to *say* about
its own trustworthiness.

This is not a hypothetical adversary. It is the ordinary case, viewed correctly: the surface
exists precisely because the thing that wrote those bytes is not trusted.

## §2 The three ways this slice can go wrong

**(a) The file impersonates the surface.** A file beginning

```
Detection: Complete
Review state: Accepted
1 file changed
```

renders today in the same font, size, column and spacing as the real lines directly above it.
After this slice it will render as three separate lines, which is *worse*, because line structure
is what made the real lines look real.

**(b) The file scrolls the truth away.** The "not a diff" label is the sentence that makes this
whole surface honest. Today it sits in the same scroll region as the content. A long enough file
pushes it off screen while the content stays.

**(c) The escaping quietly weakens.** The line break is one control character. `\r`, `\t`, ANSI
escape sequences and bidi overrides are others, and a change that "handles newlines" by relaxing
`quote_untrusted` rather than by splitting before it relaxes all of them at once.

## §3 What must be true when you are done

1. **A content line cannot be rendered where a chrome line is rendered.** Not "is styled
   differently" — cannot be. Different types, checked by the compiler, in the idiom this project
   already uses (`DisplayText`'s single constructor; `DiffContent`'s Added/Modified carried by the
   constructor rather than a field; the exhaustive `NavigationAction` matches).
2. **The framing text is outside the scroll region.** Heading, disclosure, detection status, both
   omission counts, review state, and the "not a diff" label. Only content scrolls.
3. **Every control character except the line break is still escaped**, and a test proves it with a
   fixture containing a tab, a carriage return, an ANSI escape sequence and a bidi override.
4. **Over the line bound, the preview refuses** and names which bound it hit — distinct from
   RFC-024's byte refusal, distinct from the stale-baseline refusal, distinct from both omission
   counts. Four different facts, four different sentences.

## §4 The trap this project keeps falling into

A test that asserts the right thing about the wrong property.

`0.12.0` shipped a Project Board rendering "Add Project" as an inert label for an action that did
not exist, **with a passing test over those exact strings** — it asserted they resolved to real
catalog text rather than to the raw key. A correct test of the shape of the claim, never its
truth.

The equivalent here is a test asserting that content renders as multiple lines. That is the shape.
The truth is whether a user can tell those lines from Tekstide's own, and the only test that
reaches it is one whose fixture is a genuine impersonation attempt.

**Write the spoof fixture first.** If the design makes writing it feel pointless because the
attack obviously cannot work, that is the signal that §3.1 is satisfied — and the test still ships,
as the thing that keeps it satisfied.

## §5 What you may not do to make this easier

- **Do not weaken `quote_untrusted`.** Split before escaping, not instead of it.
- **Do not answer §2(a) with "the user will notice."** The user is deciding whether to trust
  generated code, on the one screen built to help them, using their eyes. "They will notice" is
  the assumption the attack is against.
- **Do not answer §2(b) by bounding line count.** The bound exists for rendering cost (D3). A
  disclosure that survives because files happen to be short stops surviving when the bound moves.
- **Do not add a fifth "not shown" number.** This surface already distinguishes display-level from
  detection-level omission, split in `0.14.0` after a review found them summed. A refusal is not a
  count and must not be worded as one.

## §6 If the honest answer is "no"

If the container in §3.1 cannot be built such that impersonation is unrepresentable, the correct
outcome is **not** to ship line structure with a weaker guarantee.

It is to keep escaping the line break and close the defect by disclosure — the surface saying, in
its own words, that content is shown escaped and why. That is a worse product and an honest one,
and this project has taken that trade before.

Reach it by argument, written down, not by discovering the layout is hard.
