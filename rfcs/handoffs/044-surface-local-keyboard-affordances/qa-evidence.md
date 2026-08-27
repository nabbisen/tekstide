---
title: "RFC-044: Surface-Local Keyboard Affordances — QA evidence"
rfc: "RFC-044"
rfc_file: "../../accepted/044-surface-local-keyboard-affordances.md"
source_rfc_status: "Accepted 2026-08-27 — M12"
target_milestone: "M12"
created: "2026-08-27"
---

# Evidence

## PR-044-A — make the gap expressible, then count it

### The widened domain (D1)

`SurfaceAction` (`crates/tekstide/src/keyboard_help.rs`), fourteen variants across seven of the
eight surface-local handlers, keyed by (surface, action) so `Enter` meaning six different things
is six variants, not a collision. `handle_editor_key` is excluded, with the reason recorded at
the enum: its keys are continuous text-editing input, not a fixed, nameable set of discrete
actions. Arrow-key row highlighting is excluded from every handler that has it, also with the
reason recorded: it is a keyboard-native concept with no mouse equivalent to lack, so there is
nothing for `MouseOnly` to say about it.

**Closing a project is in the domain.** `SurfaceAction::TabStripCloseProject` — the specific
finding that widened this RFC's own scope.

**Response 349's required addition: a third exclusion, larger than the first two, was missing
from the record.** Modal buttons -- seven `ModalContent` variants carry real buttons with real
`on_press` handlers, `FolderBrowserChooseCurrentDirectory` among them -- are not `SurfaceAction`
entries either, and the reviewer went looking expecting a second mouse-only gap there. Verified
directly rather than taken on trust: `ModalFocusNext` (nine call sites) cycles focus generically
across whichever buttons the active modal has, and six catalog entries carry the exact on-screen
hint ("Tab/Shift+Tab moves focus; Enter activates"). Every modal button is keyboard-reachable **by
construction** -- unlike a surface, where `FocusZone`'s three variants
(`Sidebar`/`TabStrip`/`MainArea`) cycle zones, not widgets, which is the actual reason a
surface-local button can go keyboard-unreachable at all and a modal button structurally cannot.
That asymmetry is this RFC's own justification for existing, and it is now recorded at
`SurfaceAction`'s own doc comment, not only reasoned about once and left unwritten.

Surface-local keys are **not** in `KeybindingPolicy` — this slice adds nothing there at all; the
reason (a bare key would shadow every surface via `matching_global_action`) was already recorded
at that type before this RFC, and traced directly this pass: `format_binding` (`input.rs`)
returns `None` for every `Named` key (Enter, arrows, Space, Delete, Backspace, Escape — every key
all fourteen `SurfaceAction` entries actually use), so those keys are structurally immune to
global shadowing regardless; only a bare `Character` key (like a hypothetical unmodified `a`)
could ever collide, which is exactly why the two registries must stay separate.

### The mirror (D3)

`ControlCoverage` gains `MouseOnly { reason }`. `surface_keyboard_coverage(action: SurfaceAction)
-> ControlCoverage`, exhaustive, is the required mirror of `control_coverage` — that function
still only asks "how does a *mouse* reach this" of `NavigationAction`, unchanged; the new one
asks "how does the *keyboard* reach this" of the wider domain. Ablated exactly as the checklist
asks: added an undecided fifteenth variant, both this function and its own handler-name lookup
failed to compile, naming the missing variant; removed, clean build restored.

**`VisibleControl`'s shape reused for the keyboard question, `KeyboardOnly` deliberately not.**
Recorded explicitly at both the enum and the function: `KeyboardOnly` already has an established
meaning ("no mouse control exists, keyboard is the only route") from `control_coverage`'s own use
of it; reusing it here to mean "a keyboard route exists" would silently invert that meaning for
readers who know the original. `VisibleControl`'s two-field shape (a description plus a literal,
grep-checked source snippet) generalizes cleanly to "a real trigger exists via this function's own
input method" regardless of which method that is, so it is reused as-is; `MouseOnly` is the one
genuinely new arm, and it requires a reason exactly as `KeyboardOnly` already does — checked by a
real test (below), not merely documented.

### Not enforced by a source scan, in the sense that phrase warns against

D3's registry does not drive real dispatch — checked directly, not assumed. `FocusZone` has three
variants (`Sidebar`/`TabStrip`/`MainArea`); the one `RoutedInput::Surface` arm in `update()` calls
all six `MainArea`-consuming handlers unconditionally in sequence, and each self-guards via its
own `open_surface()`/`route()` check. There is no dispatch table keyed by "current surface" for a
registry to hook into without restructuring that call site. **Reported per D3's own explicit
instruction** rather than silently falling back: dispatch-through-registry is impractical given
this structure.

The enforcement mechanism is instead the same shape `control_coverage`/
`every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry` already established and
had accepted: an exhaustive match (compile-time enforcement that every action is decided) cross-
checked against real source at test time. This *is* a text scan in the literal sense, but not the
kind RFC-042's failed guard was — that guard checked for a syntactically flexible construct
(`scrollable(column(lines))` vs `scrollable(column![...])`, many equivalent spellings, trivially
respelled around). A key match (`keyboard::key::Named::Delete`) has one canonical, idiomatic
spelling in this codebase; there is no equivalent-but-differently-spelled way to write it that
would defeat the check while keeping the same behavior. The new check goes further than
`control_coverage`'s own version besides: several `SurfaceAction` entries share the identical
literal snippet (`"keyboard::key::Named::Enter"` names six different actions across six different
handlers), so a whole-crate `.contains()` — sufficient for `control_coverage`, since its own
snippets are each unique — would not actually prove anything for a shared one. The new check
extracts each claimed handler's own function body by brace-counting and requires the snippet
inside *that* body specifically, so a claim naming the wrong handler, or naming a handler whose
key match was removed, still fails.

### The inventory, and the count

Fourteen `SurfaceAction` entries; **one** `MouseOnly` (`TabStripCloseProject`, `TrackedGap`); the
other thirteen are `VisibleControl` — already keyboard-reachable, none of the thirteen advertised
anywhere yet (PR-044-C's own job). Nobody had this number before this slice.

### Gate chosen: red, deliberately

`surface_action_inventory_has_no_unclosed_tracked_gaps` asserts zero unclosed `TrackedGap`
entries and fails today, naming `TabStripCloseProject` and its own reason. Chosen over a passing
test that merely prints the count, mirroring PR-043-A's own precedent exactly: a slice whose whole
point is making a gap countable should not also hide the count behind green. Stable red across
three consecutive full-workspace runs — same single failure, same message, every time.

**Also verified**: `every_surface_action_has_a_checked_keyboard_route_or_a_reasoned_mouse_only_entry`
passes — the thirteen real routes are each confirmed present in their own claimed handler's body;
`TabStripCloseProject`'s `MouseOnly` reason is non-empty. Ablated (fabricated one entry's snippet):
failed, naming the entry and the handler it was falsely claimed to be found in. Restored: passes.

`control_coverage`'s own existing cross-check (`every_live_action_has_a_visible_control_or_a_reasoned_allow_list_entry`)
needed one addition: `ControlCoverage` becoming a three-arm enum broke its exhaustive match. Added
a `MouseOnly` arm there that panics if ever reached, since a `NavigationAction` is required to
carry a live `KeybindingPolicy` rule to exist in that domain at all — `MouseOnly` should be
structurally unreachable from that function, and the panic states why rather than silently
matching it as if it were expected.

### Gate

`fmt`, `clippy -D warnings`, `git diff --check`: clean. Three consecutive full-workspace runs:
450 passed + 1 failed (the deliberate one) every time, stable. `rfc_docs_invariants`: clean.

## PR-044-B — close the access gap

**`TabStripCloseProject` closed, the only entry the inventory found, first per the handoff's own
required ordering.** `handle_tab_strip_key` gains one arm: `Delete`, guarded on
`state.tab_strip_highlight > 0` (a real project tab highlighted, not the "Projects" board entry at
index 0, which `×` itself has no button for either), calling the same `attempt_close_project_tab`
the `×` button already reaches.

**`Delete` confirmed unclaimed before use**, the same check `handle_trust_settings_key`'s own doc
comment records making: `handle_tab_strip_key`'s own match, before this change, used only
`ArrowRight`/`ArrowLeft`/`Enter`. Reused across surfaces safely regardless (`handle_trust_settings_key`
already uses `Delete` too, for a different, mutually-exclusive surface) — `FocusZone`/`open_surface`
guard the two apart structurally.

`SurfaceAction::TabStripCloseProject`'s own coverage entry moved from `MouseOnly` to
`VisibleControl`. `ControlCoverage::MouseOnly` itself now has zero live constructors —
`#[allow(dead_code)]`'d rather than removed, since §6 of `what-advertising-keys-must-not-become.md`
explicitly anticipates a future, legitimate mouse-only control needing it again; the reason is
recorded at the variant.

### Required test, through the real message path

`delete_on_a_highlighted_project_tab_closes_that_project`: two real projects, `Delete` dispatched
through `Message::Input(RoutedInput::Surface(...))` and `super::update()` — the same shape
`enter_on_a_highlighted_project_tab_switches_to_that_project` already uses for its own sibling
action, not a direct call to `attempt_close_project_tab` in isolation. Asserts the highlighted
project is genuinely gone (`state.app_shell.state().project(&id).is_none()`), the other project is
untouched, and no confirmation modal opened (an idle project is `SafeToClose`). A second test,
`delete_on_the_highlighted_home_tab_is_a_no_op`, proves `Delete` on the board entry (index 0) does
nothing, the same "guard, then act" shape every sibling handler already uses.

**Ablated**: removed the new `Delete` arm, reran — `delete_on_a_highlighted_project_tab_closes_that_project`
failed, naming the exact assertion ("the highlighted project must be closed for real, not merely
have a modal opened over it"). Restored: passes.

### Live GUI evidence

Release binary, a fresh `mktemp -d` fixture project (`/tmp/tmp.1pqCFhr1A1`) and a fresh
`mktemp -d` `XDG_STATE_HOME`, launched with `WAYLAND_DISPLAY` unset to force the X11/XWayland
backend (this repo's own established convention). `xdotool key --clearmodifiers` for every input
below; `niri msg action screenshot-window --id <id>`, targeted by window id, so the owner's own
desktop focus was never touched.

**Zero mouse clicks, start to finish.** `Tab` ×3 (cycling `FocusZone` from wherever the app booted
into around to `TabStrip`, visible as a blue border around `[Projects]`), `Right` ×1 (moves the
highlight onto the real project tab, `EVIDENCE-1-project-tab-highlighted-by-keyboard.png`),
`Delete` ×1. **No real mouse click was sent** — stated plainly, per the RFC's own requirement
either way.

Result (`EVIDENCE-2-project-closed-tab-gone.png`): the project's own tab is gone from the strip
entirely; its Project Board row now shows an "Open" button instead of `×`, the same transition a
real close via the mouse button produces. Closed directly, no confirmation modal, matching the
idle/`SafeToClose` case the required test above also covers -- the live-work/confirmation-modal
case is the same path RFC-043's own evidence already exercised (that dialog is already
keyboard-reachable, Tab/Shift+Tab/Enter, from that RFC's own work), so this evidence deliberately
covers the *other* half: the case that previously had no keyboard route into the close sequence at
all.

Two screenshots, `rfcs/handoffs/044-surface-local-keyboard-affordances/evidence/`. Neither shows a
path under `$HOME`, a real project name, or another project on screen.

### Gate

`fmt`, `clippy -D warnings`, `git diff --check`, `rfc_docs_invariants`: clean. Three consecutive
full-workspace runs: **453 + 4 + 746, fully green** -- the deliberate PR-044-A failure is gone,
closed by this slice, not merely raised.

## PR-044-C

Not started.
