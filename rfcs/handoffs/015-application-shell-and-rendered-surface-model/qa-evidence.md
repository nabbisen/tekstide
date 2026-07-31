# RFC-015: Application Shell and Rendered Surface Model - QA Evidence

Status: Proposed — implementation in progress (PR-015-B landed 2026-07-31, not yet reviewed; PR-015-C onward pending)
Date opened: 2026-07-29
Date accepted: Pending

## Scope

RFC-015 builds the application shell and defines the rendered-surface contract, layer model, and input routing that RFC-017, RFC-019, RFC-020, and RFC-022 all build on.

Evidence in this file must not be used to claim terminal rendering, editor/explorer/diff/report surfaces, real security dialogs, locale catalogs, configuration loading, multi-window support, or screen-reader accessibility — unless later reviewed implementation explicitly supports that claim.

## Inherited obligations

Carried in from RFC-014's approved decision record:

- **R1 — latency unverified.** C2/C3/C4 were not measured; `iced::window::frames()` forces continuous redraw once subscribed. **PR-015-F discharges this or re-records the residual honestly.** Another all-zero figure is not an acceptable outcome.
- **R6 — focus-trap property does not transfer.** The spike's property held only because its terminal emitted no messages. **PR-015-C must re-establish it under an input-accepting design**, with a real test rather than a structural argument.
- **R2 — no screen-reader support**, owner-accepted 2026-07-29. Public claims must state the limitation; no simulated affordance.
- **R9 — survivorship bias** in confirmed-only percentiles applies to any reused synthetic-input harness.

## Design Review

Pending PR-015-A acceptance.

## Implementation Evidence

### PR-015-B — Window, layers, chrome, seams

**Scope followed.** `crates/tekstide` is now a real `iced` (0.14) application; `main.rs`'s text harness (`print!("{}", shell.render_text())`) is gone. `crates/tekstide/src/shell.rs` owns layer composition and the (currently uninhabited) `Message`/`update`/`view`; `crates/tekstide/src/theme.rs` owns the `Theme` seam; `crates/tekstide/src/main.rs` owns process boot (recent-project restore, CLI project-path arguments, catalog resolution), matching `implementation-handoff.md` §1's suggested layout.

**This slice is deliberately non-interactive.** `shell::Message` is an uninhabited enum (`pub enum Message {}`) and `shell::update` is unreachable (`match message {}`). No keyboard subscription exists. This is not an oversight: PR-015-C is the slice that introduces any input at all, and `pr-015-c-input-routing.md` is explicit that a slice inventing its own ad hoc key handling ahead of that document is exactly the shape of mistake it warns against. There was therefore nothing for this slice to route.

**Layer composition** — chrome (top bar, status bar) / content (placeholder; no surface exists until PR-015-D) / modal, composed via `stack`/`opaque`, the mechanism RFC-014 C8 proved. The modal layer's only occupant in this slice is a placeholder demo, per `implementation-handoff.md` §8's explicit allowance ("a placeholder dialog for testing the layer is fine and should be clearly marked as such"). It is env-gated (`TEKSTIDE_LAYER_DEMO`, read once at boot), not keyboard-gated — the same convention RFC-014's own spike used for its measurement/demo flags (`TEKSTIDE_MEASURE_CRITERION`, `TEKSTIDE_I18N_DEMO`) — specifically so this slice adds no input path of its own.

**Theme seam.** `theme::Theme` (background/foreground/accent/border/surface colours, three font sizes), compiled default via `Default`. No `border_focused` field: nothing in this slice has a focus concept yet (no surfaces, no input routing), so a field with no caller was removed rather than shipped dead — the same discipline `LocalePreference`'s ahead-of-caller fields are held to elsewhere in this codebase (those are still *reachable*, just always `None` today; an unused method is a different, weaker case and was cut).

**i18n seam.** Wired through the already-reviewed `i18n::Catalog`/`i18n::CatalogArgs` (RFC-016 PR-016-B/D, response 126 approved). New keys added to `en.ftl`: `content-area-placeholder-title`, `content-area-placeholder-body`, `status-bar-summary` (a two-part key: a literal-variant route selector plus a genuine plural count, in one lookup — the same one-key pattern PR-016-D established for `blocked-automation-count`), and the three `layer-demo-modal-*` keys. Not mirrored into `pl.ftl`: those keys are chrome, not plural-machinery content, and `pl.ftl` exists only to prove CLDR plural-category selection (RFC-016 §Non-Goals); leaving them absent there deliberately exercises the real fallback-to-source-locale path rather than being an oversight.

**Seam enforcement — mechanical, per the review gate's stated preference.** Three heuristic scans (`shell::tests`), each ablation-verified by temporarily reintroducing the violation and confirming the specific test fails, then reverting:
- `no_raw_string_literal_is_passed_to_text_anywhere_in_the_crate` — no `text("literal")` call anywhere scanned; every string comes from `state.catalog.get(...)` or a helper that does.
- `no_raw_color_construction_anywhere_in_the_crate` — no `Color::from_rgb`/`from_rgba` anywhere scanned; every colour comes from `state.theme`.
- `no_raw_font_size_literal_anywhere_in_the_crate` — no bare numeric literal passed to `.size(...)` anywhere scanned; every size comes from `state.theme.font_size_*()`.

**Response 128 Required, fixed:** the original scans named `shell.rs` directly, so `main.rs` was unscanned then, and PR-015-C's routing module and PR-015-D's surface modules would have landed unscanned later — silently, since a passing test reads as coverage. Fixed by walking `crates/tekstide/src` recursively (`scannable_source_files`), so a new source file anywhere in the tree is scanned automatically, with no list to fall out of date. Two stated exemptions: `theme.rs` (the seam's own implementation — it is what other files must source colours *from*, not a violation) and any `tests.rs` (test code legitimately contains literals that are not user-facing shell output). Re-ablation-verified after the fix: temporarily added a raw `text("literal")` call to `main.rs` — previously unscanned, now caught (`main.rs:20 passes a string literal directly to text(...)`); reverted immediately.

These are heuristics over each file's own text, not a full parse — recorded as a limitation, not a full mechanical guarantee (a literal split across an expression the scan doesn't recognize could slip past). No hardcoded-strings scan exists yet at the `tekstide-core::shell::render_text` level; that is RFC-016 PR-016-E's job. Response 128 flagged that this scan and PR-016-E's future scan will overlap in policy (one covers `crates/tekstide`, the other will cover `tekstide-core`) — noted in RFC-016's handoff as a consolidation point for whichever lands second, not solved here.

**Behavioral tests**, decomposed so the underlying string logic is testable without going through `iced`'s `Element` tree (`status_bar_summary` returns a `String`, called by both `view` and the tests directly):
- `window_title_resolves_through_the_catalog_not_a_literal` — proves the title comes from a real catalog key (`Catalog::get`'s "missing key renders as the key itself" fallback would fail this loudly if the key name were ever mistyped).
- `status_bar_summary_reflects_the_default_route_and_zero_projects` and `status_bar_summary_pluralizes_a_single_project_correctly` — the route-label/plural-count summary resolves correctly at zero and at exactly one project (the English singular/plural boundary), reusing PR-016-D's own plural machinery through a real, non-numeric-plus-numeric one-key lookup (`status-bar-summary`, structured like `blocked-automation-count`). Both assertions include Fluent's automatic bidi isolate marks explicitly (two select-expression placeables in one pattern each get isolated, with the inner `{$count}` isolated a second time nested inside) — the same accepted double-isolation response 125 ruled on, arising here from adjacent select expressions rather than `CatalogArgs::untrusted`, and asserted literally rather than stripped, matching `i18n::tests`' own convention.

**No GUI dependency in `tekstide-core`, verified not assumed.** `cargo tree -p tekstide-core --edges normal | grep -i iced` returns nothing.

**No shell-local state mirrors core state.** `shell::State` holds exactly one `ApplicationShell` (the sole source of model state) plus purely presentational fields: `catalog`, `theme`, and `layer_composition_demo_modal_open` (this slice's only "which zone has focus, whether a modal is open"-shaped field, and it is scaffolding, not a real modal occupant). This is recorded by inspection, per `implementation-handoff.md` §2's own examples of what counts as legitimate shell-local state — there is no automated check for "no field secretly duplicates a core value" and none is claimed.

**Screenshot evidence** (response 127's standing convention: `niri msg action screenshot-window --id <id> --path <repo-relative-file>`, targeted by window ID so the owner's desktop focus was never touched — no `focus-window` call was made or needed, since this slice has no input to deliver):

- `evidence/pr-015-b/shell-chrome-over-real-state.png` — the shell running with no projects: window titled "Tekstide" (confirming the i18n-sourced window title), top bar, content placeholder, status bar reading "Project Board | 0 projects" (confirming `status_bar_summary` against real, empty `ApplicationShell` state).
- `evidence/pr-015-b/layer-composition-demo-modal-above-content.png` — `TEKSTIDE_LAYER_DEMO=1`: the placeholder dialog renders centered, above the content area, chrome still visible above and below it — the `stack`/`opaque` composition holds.

**A property this slice established, not only tested (response 128): `iced` honours Fluent's automatic bidi isolation.** `status_bar_summary` demonstrably produces `⁨Project Board⁩ | ⁨⁨0⁩ projects⁩` (isolate marks and all, asserted literally in `shell::tests`), and `shell-chrome-over-real-state.png` shows it rendering clean as `Project Board | 0 projects` — no stray glyphs, no tofu. `iced`/`cosmic-text` consumes U+2068 (First Strong Isolate) and U+2069 (Pop Directional Isolate) as directional instructions rather than drawing them visibly. This closes RFC-016 `qa-evidence.md`'s open limitation ("hasn't been evaluated against every possible downstream renderer... no such renderer exists yet") — one now exists, on this exact interpolated string, and it handles the marks correctly. Not claimed here: that *every* possible interpolated string or every other rendering path in `iced` behaves identically — only that the one real, shipped case this slice renders does.

**What these screenshots prove, and what they must not be cited for** (response 127, so a future closeout does not overclaim from them):
- They prove Tekstide's own shell — not the RFC-014 spike — actually boots on `iced`, renders chrome over real `ApplicationShell` state, and composes a layer above content. That is "the product is wired up," and nothing more.
- **They do not re-prove substrate modal composition.** RFC-014 PR-014-D already did that, with a stronger artifact (`evidence/pr-014-d/genuine-and-adversarial-dialog-one-frame.png`: a real modal beside an adversarial terminal imitation in one frame). A placeholder box here adds nothing to that claim.
- **They do not make the RFC-009 §212 trusted-UI claim.** That claim is about a trusted dialog composited above *untrusted terminal output*; there is no terminal surface until RFC-017, and no untrusted content renders anywhere in this slice. The security-relevant version of this evidence lands in RFC-018, not here.

Gates run 2026-07-31: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 23 `tekstide` — up from 15, 8 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

### PR-015-C — Input routing and focus model

**The security-critical slice.** `pr-015-c-input-routing.md` read first, as instructed. New module `crates/tekstide/src/input.rs` (+ `input/terminal_surface.rs`) owns classification; `shell.rs` owns turning a classified input into a real state change.

**The three input classes, as distinct types, per the required structure:**
- `ShellInput(NavigationAction)` — private tuple field, defined in `input.rs`. Only `route_non_modal_input` (in the same module) constructs one.
- `SurfaceInput { target: FocusZone, key: KeyPress }` — private fields, same module.
- `TextStream` — defined in `input::terminal_surface`, a *submodule* of `input`. Its sole constructor, `from_terminal_key`, is `pub(super)` rather than `pub(crate)` — deliberately narrower than the crate-visibility level, because `pub(crate)` would let a future `crates/tekstide/src/surface/*` module (PR-015-D) construct one directly, exactly the bypass this type exists to close. `pub(super)` permits only `input` (the router) and `input`'s own descendants (its `#[cfg(test)]` code) — never `shell.rs`, `main.rs`, or a surface module.

**Guard-deletion resistance, made structural via a proof token, not asserted by convention.** `route_non_modal_input` requires an `input::ModalAbsent` argument. The only constructor is `ModalAbsent::check(&Option<T>)`, which inspects the real modal state at the call site; `ModalAbsent`'s single field is a private `()`, so there is no other way to produce one. Deleting the `match input::ModalAbsent::check(&state.modal) { ... }` guard in `shell::subscription` and calling `non_modal_subscription` unconditionally is therefore not a runtime behaviour change — the compiler has no `ModalAbsent` value to pass, since none can be constructed outside `input`. Same "make the invalid state unrepresentable" pattern already used for `DisplayText`, `VerifiedCwd`, `RunCapabilityToken`, and `CatalogArgs` (RFC-016 response 125), applied here to a routing decision.

**Three compile-fail probes, run and confirmed, not asserted from reading the code** (manual probe-then-revert, `tekstide` has no `[lib]` target for a permanent `compile_fail` doctest — same disclosed limitation as `CatalogArgs`, response 125):
1. `crate::input::TextStream::from_terminal_key(...)` called from `shell.rs` → `error[E0624]: associated function 'from_terminal_key' is private`.
2. `crate::input::ShellInput(NavigationAction::OpenProjectBoard)` called from `shell.rs` → `error[E0603]: tuple struct constructor 'ShellInput' is private`.
3. `non_modal_subscription(input::ModalAbsent(()), state.focus)` called from `shell.rs`, bypassing `ModalAbsent::check` entirely (the literal "delete the guard" scenario) → `error[E0603]: tuple struct constructor 'ModalAbsent' is private`.

All three probed by temporarily adding the offending code to `shell.rs`, confirming the exact compile error, then reverting — `git diff --check` clean afterward.

**Response 130 Required 1 — corrected: the *call gate* is structural; *exclusivity* additionally depends on `iced`'s subscription lifecycle, and that dependency was unstated.** The original wording here claimed "modal exclusivity is structural... not 'checked'" without qualification. That overstates what the type proves. `route_non_modal_input` genuinely cannot be called without a `ModalAbsent`, and none can be constructed outside `input` — the *call* is gated structurally, confirmed above. But `ModalAbsent` is `Copy` (required by `.with()`'s `Hash` bound, so it can be threaded into a long-lived subscription closure), which means a proof obtained once can outlive the instant it was true. The architect probed this directly:
```
modal active = true
routed anyway with a stale proof -> Surface(SurfaceInput { target: MainArea, ... })
SurfaceInput produced while a modal is open? true
```
**A captured proof does produce `SurfaceInput` while a modal is active.** This is not exploitable today: `shell::subscription` re-evaluates on each rebuild and returns `modal_subscription()` once `state.modal` is `Some`, so the non-modal subscription — and its captured proof — is torn down by `iced` before that stale proof could ever be used against a real key event. **But the property holds because of `iced`'s subscription-rebuild lifecycle, not because of the type alone.** That is the corrected claim: the call gate is structural (type-level, tested by the three compile-fail probes above); full exclusivity is that gate *plus* a framework-lifecycle assumption that `iced` actually tears down a dropped subscription branch promptly. This matters beyond bookkeeping because RFC-022's approval-dialog property ("no keystroke reaches a PTY while a dialog is open") rests on exactly this chain, and RFC-017 will put real PTY keystrokes through it — so it needs to inherit an accurate description of what it depends on, not an overstated one.

To make at least the branch-selection half assertable rather than left implicit, the decision is now extracted into `input::SubscriptionMode`:
```rust
pub enum SubscriptionMode {
    NonModal(ModalAbsent),
    Modal,
}
impl SubscriptionMode {
    pub fn for_modal<T>(modal: &Option<T>) -> Self { /* wraps ModalAbsent::check */ }
}
```
`shell::subscription` matches on `SubscriptionMode::for_modal(&state.modal)`; `shell::tests::subscription_mode_reflects_whether_a_modal_is_active` asserts directly that a `State` with a modal yields `Modal` and one without yields `NonModal(_)` — converting that half from an implicit assumption into a tested one, without touching the routing design or fighting `Subscription`'s opacity (`Copy` was kept on `ModalAbsent`, per the architect's explicit instruction not to remove it — `.with()` needs it, and removing it would fight the framework for no gain now that the branch itself is tested).

`modal_subscription()` remains a *separate* function producing only `Message::Modal*` variants — it has no path to constructing `RoutedInput::Surface` or `RoutedInput::Terminal` at all. While a modal is shown, the subscription that could produce those is not subscribed (modulo the framework-lifecycle dependency named above), matching the required property's language: "not produced," not "produced and discarded."

**Precedence, and the one deliberate simplification recorded rather than silently resolved.** `route_non_modal_input` checks, in order: (1) a key matching a live `KeybindingPolicy` rule — global keybindings always win, proven with a terminal nominally focused (`a_global_keybinding_wins_over_a_focused_terminal`); (2) Tab/Shift+Tab, the shell's own focus cycle; (3) `terminal_focus`, if set; (4) otherwise `SurfaceInput` for the focused zone. Tab is checked *ahead of* terminal focus deliberately — whether Tab should instead reach a terminal's text input (shell completion, etc.) is a real, currently unanswerable question with no terminal surface yet to decide it against; recorded for RFC-017, not resolved here (this is RFC-015's Open Question 2, "raw key events or pre-interpreted intents" — answered narrowly: raw `KeyPress` for now, since the actual raw-vs-intent tradeoff has no real surface yet to weigh it against; deferred to whichever of PR-015-D/RFC-019 first needs to decide).

**The `TerminalId` liveness check** (`pr-015-c-input-routing.md` requirement 2: "a stale or cross-project id is dropped, not best-effort delivered") lives in `shell::terminal_stream_targets_a_live_terminal`, checking `app_shell.state().active_project().and_then(|p| p.terminal_session(id)).is_some()`. **Only the negative path is testable from `tekstide`'s own test suite today**, and this is a real, disclosed gap, not a shortcut: attaching a genuinely live `TerminalSession` needs `tekstide_core::AppState::project_mut`, which is `#[cfg(test)]`-gated *inside `tekstide-core` itself* with zero production call sites anywhere in the tree (`grep`-confirmed — every `add_terminal_session` caller is a `tekstide-core` test; terminal creation is RFC-017's job). `#[cfg(test)]` does not cross the crate boundary, so `tekstide`'s tests cannot reach it, and widening `tekstide-core`'s API to manufacture a fixture would itself be "a change to `tekstide-core` state models without raising it first" (`implementation-handoff.md` §8) — not something to do unilaterally to make a test easier. Proven instead: a never-existed `TerminalId` against a real (empty) active project (`a_never_added_terminal_id_is_not_live`), and no active project at all (`with_no_active_project_no_terminal_id_is_ever_live`) — both correctly `false`. The positive case becomes testable the moment RFC-017 gives `tekstide-core` a real way to attach a terminal.

**No PTY-writing path exists anywhere yet** (RFC-017 has not landed) — `shell::update`'s handling of `RoutedInput::Terminal` only exercises the liveness check and discards the result; there is nothing to deliver to or drop from in practice. The type and the check are proven correct now so RFC-017 inherits a working, tested contract rather than discovering the check wrong once it matters.

**A real focus-trap test, discharging RFC-014 R6** — not a structural argument. `shell::tests::modal_focus_cycling_never_touches_the_shell_focus_cycle` dispatches real `Message::ModalFocusNext`/`ModalFocusPrevious` through `update` while `state.modal` is `Some`, and asserts both that the modal's own `ModalButton` cycles correctly (Acknowledge ↔ Dismiss) *and* that `state.focus` (the shell's own focus cycle) never moves. `dismissing_the_modal_clears_it_and_leaves_shell_focus_undisturbed` proves dismissal (`Enter`/`Escape` → `ModalActivate`/`ModalDismiss`) clears the modal. Focus-return-to-invoking-element (UI/UX §18) falls out for free rather than needing separate restore logic: because `state.focus` was never touched while the modal was shown (proven by the same test), whatever it held before is simply still there after dismissal.

**The modal became genuinely dismissible.** PR-015-B's placeholder (env-gated, never closing) could not exercise either property above — a modal that never closes cannot prove exclusivity ends correctly or that focus returns anywhere. `ModalContent { focus: ModalButton }` now has two real targets (`Acknowledge`/`Dismiss`, mirroring the RFC-014 spike's `DialogButton` shape), Tab/Shift+Tab cycles between them, Enter/Escape dismisses. Still explicitly scaffolding (`implementation-handoff.md` §8's placeholder allowance) — RFC-022 supplies real dialogs; opening this one remains `TEKSTIDE_LAYER_DEMO`-gated, since there is still no real trigger to open a dialog, only now a real way to close one.

**`ShellInput` dispatch is real, and honestly partial.** `app_command_for` maps `NavigationAction::OpenProjectBoard` to `AppCommand::OpenProjectBoard` — the only pairing where both a live default keybinding (`KeybindingPolicy::linux_mvp()`) and an existing `AppCommand` exist today. `OpenCommandPalette` has a real, reserved binding (`Ctrl+Shift+P`) but no command-palette feature to dispatch to yet; every other `NavigationAction` has no default binding at all until RFC-023. `a_project_board_shell_input_dispatches_the_real_app_command` proves the dispatch is genuine: starts from `ActiveProjectWorkspace`, feeds a real `ShellInput` through `update`, confirms the route actually changed via `status_bar_summary`.

**Focus cycling exists and is proven, even though it is a no-op today.** `FocusZone` has a single real variant (`MainArea`, `#[non_exhaustive]`) — PR-015-B built no sidebar (PR-015-E's scaffolding). `focus_next_and_previous_route_through_update` proves `FocusNext`/`FocusPrevious` route through `update` correctly now, so the day PR-015-E adds `Sidebar`, this test either still passes trivially or fails pointing at exactly what needs updating.

**No `tekstide-core` changes.** All of the above is new `tekstide` code; nothing in `crates/tekstide-core` was touched.

Gates run 2026-07-31 (original submission): `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 35 `tekstide` — up from 23, 12 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Response 130 fixes, gates re-run 2026-07-31:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-targets --all-features` (490 `tekstide-core` + 37 `tekstide` — up from 35, 2 net new — + 18 `tekstide-gui-spike`, 0 failures), `git diff --check` — all passed.

**Screenshot evidence, with real synthetic input.** Flagged before running, per response 129's explicit instruction, and the owner approved it explicitly before any command ran. Captured exactly per RFC-014's precedent: relaunched with `WAYLAND_DISPLAY` unset (`env -u WAYLAND_DISPLAY TEKSTIDE_LAYER_DEMO=1 ./target/debug/tekstide`) to force the X11/XWayland `winit` backend, `xdotool search --name Tekstide` for the X11 window id, `xdotool windowfocus <id>` (required here — a first attempt via `key --window <id>` alone, without focusing, delivered nothing; `windowactivate` failed outright since niri's `XWayland` support does not answer `_NET_ACTIVE_WINDOW`, but plain `windowfocus` worked), then `xdotool key --window <id> <Key>`. Screenshots captured by niri's own window id (`niri msg action screenshot-window --id <id> --path <file>`, response 127's convention) — a *different* id namespace than xdotool's X11 id, confirmed by cross-checking `niri msg windows`.

- `evidence/pr-015-c/modal-1-initial-dismiss-focused.png` — modal open (`TEKSTIDE_LAYER_DEMO=1`), `ModalContent::default()`'s `Dismiss` focused (`>` marker), matching the code.
- `evidence/pr-015-c/modal-2-tab-to-acknowledge-focused.png` — after one real `Tab` keystroke: focus moved to `Acknowledge`.
- `evidence/pr-015-c/modal-3-tab-cycles-back-to-dismiss.png` — after a second real `Tab`: focus cycled back to `Dismiss`.
- `evidence/pr-015-c/modal-4-escape-dismisses.png` — after a real `Escape` keystroke: the modal is gone, content area and chrome visible underneath, undamaged.

**The cycle proof (response 130), stated precisely, not just "byte-different."** `md5sum` of all four:
```
modal-1-initial-dismiss-focused.png        0ce3842440110517b7f51cb963309fc4
modal-2-tab-to-acknowledge-focused.png     ed18602bd19b1746522409da655e2558
modal-3-tab-cycles-back-to-dismiss.png     0ce3842440110517b7f51cb963309fc4
modal-4-escape-dismisses.png               f359fcdcd86c3434c3753364c844000e
```
**1 and 3 are byte-identical; 2 differs from both.** If the second `Tab` press had done nothing, 3 would equal 2, not 1. It equals 1 — focus genuinely traversed to `Acknowledge` and back to `Dismiss`. That is proof of a real two-way *cycle*, the property `modal_focus_cycling_never_touches_the_shell_focus_cycle`'s name actually claims, not merely proof that a keystroke changed something on screen.

**What these screenshots prove, and what they do not** (same discipline as response 127 applied to PR-015-B's): they prove the modal's Tab-cycle and Escape-dismiss genuinely work end-to-end through real input, on this exact `iced` substrate. They do not prove modal *exclusivity* (that surface/terminal input is *not produced* while the modal is shown) — that property is structural (`ModalAbsent`, a different subscription function entirely) and is proven by code inspection and the compile-fail probes above, not by a screenshot; there is no surface or terminal yet whose non-delivery a screenshot could show either way.

### PR-015-D — Project Board surface

Pending implementation.

### PR-015-E — Mode switching and Content-mode scaffolding

Pending implementation.

### PR-015-F — Measurement: R1 discharge

Pending implementation.

### PR-015-G — Closeout evidence

Pending implementation.

## Known Limitations

- Screen-reader support absent for the life of the `iced` substrate decision (RFC-014 R2, owner-accepted).
- Terminal rendering, editor, explorer, diff, and report surfaces are out of scope; the shell provides only the contract they plug into.
- The modal layer ships with a placeholder dialog for testing; real dialogs are RFC-022.
- **The "genuinely live terminal" positive path of `terminal_stream_targets_a_live_terminal` is untested from `tekstide`'s own suite** — `tekstide_core::AppState::project_mut` is `#[cfg(test)]`-gated to `tekstide-core`'s own crate boundary, and no production call site attaches a `TerminalSession` anywhere in the tree yet (RFC-017's job). Only the negative (not-live) path is proven today; recorded as a real gap, not hidden behind a fixture that only exercises the easy half.
- **No post-dismissal keystroke queuing is tested** — the current subscription design has no queue to test draining from (each `subscription()` call is stateless), so "no post-dismissal delivery" is a property of the design shape rather than something separately verified by a test.
- **`xdotool windowactivate` does not work under this niri/XWayland setup** (`_NET_ACTIVE_WINDOW` unanswered); `xdotool windowfocus` does. Recorded for the next slice needing synthetic input (PR-015-E onward) so it isn't rediscovered.
- `SurfaceInput`'s payload has no real consumer yet (PR-015-D); `shell::update` receives and discards it, proven only at the routing layer (`input::tests`).
- `format_binding`'s modifier ordering (Ctrl, Alt, Shift) matches the two bindings `KeybindingPolicy::linux_mvp()` currently ships; a future binding combining modifiers in an order this function doesn't anticipate would need the function extended, not the policy.
- **Response 130 Recommended, closed: `format_binding` only handles `Key::Character`.** A `KeybindingPolicy` rule bound to a named key (`F1`, `Escape`, `Delete`) would make `matching_global_action` return `None` for it, silently falling through to `SurfaceInput` and quietly defeating "global keybindings are not capturable by a surface" for that one binding — no live gap today (both real bindings are single-character), but a real one the day a named-key binding is added. `input::tests::every_default_binding_in_linux_mvp_round_trips_through_format_binding` now asserts every real `default_binding` round-trips through `format_binding`; ablation-verified by temporarily giving `OpenSafeCloseDialog` a `Some("F1")` binding in `tekstide-core` — the test failed with a clear message pointing at the unhandled named key — then reverted (`tekstide-core` untouched in the committed diff).
